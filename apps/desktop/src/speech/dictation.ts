import { captureFailureKey, classifyCaptureError, type AudioCapture } from "../audio/recorder";
import { normaliseError } from "../lib/errors";
import type { SpeechTranscript } from "./ipc";
import {
  canUndo,
  countNumbers,
  insertTranscript,
  rememberSelection,
  type Insertion,
  type RememberedSelection,
} from "./insertion";
import type { VoiceSession } from "./voiceSession";

/**
 * The dictation in the question box, as a state machine with no React and no Tauri: the recorder,
 * the transcription, the clock and the voice session are handed in, so every transition and every
 * cancel is tested without a window. A transcript only fills the box; this file never sends a
 * question, never keeps the audio and never reaches the network.
 */

export type DictationPhase =
  | "unavailable"
  | "idle"
  | "requesting-permission"
  | "recording"
  | "transcribing"
  | "done"
  | "error";

/** Shorter than this is a click, not a sentence; refused before any engine call. Rust refuses under 0.3 s. */
export const MIN_DICTATION_SECONDS = 0.4;
/** A peak under this share of full scale is a muted or covered microphone. Equals `QUIET_PEAK_FLOOR`
 * of `speech/wav.rs`; a test reads that file and fails if the two drift apart. */
export const QUIET_PEAK_FLOOR = 0.01;
/** A clip with more than this share of saturated samples is transcribed with a warning. */
export const CLIPPED_SHARE_WARNING = 0.005;
/** The recording stops this long before the cap, so the clip (which runs a little past the press of
 * the button) is never over the cap Rust enforces. */
export const CAP_MARGIN_MS = 1_000;

/** A sentence to show: a catalogue key and the values it needs. */
export interface DictationNote {
  key: string;
  values: Record<string, string | number>;
}

/** What the last successful dictation left in the box, for the status line and the Undo. */
export interface DictationResult {
  insertion: Insertion;
  numbers: number;
  clipped: boolean;
  /** The engine that heard the words, and whether it writes capitals where a name has them. */
  engineId: string;
  caseReliable: boolean;
}

export interface DictationSnapshot {
  phase: DictationPhase;
  /** Set in the `error` phase only. */
  error: DictationNote | null;
  /** Set in the `done` phase only. */
  result: DictationResult | null;
  /** The caret to restore after the box changed. `nonce` changes with every new position, and `text`
   * is what the box must hold for the position to mean anything (the draft and this snapshot reach
   * the screen in either order). */
  caret: { position: number; nonce: number; text: string } | null;
}

export interface DictationDeps {
  /** A new recorder for each recording. */
  capture: () => AudioCapture;
  /** Sends the WAV to the engine. Rejections are machine codes. */
  transcribe: (wav: Uint8Array) => Promise<SpeechTranscript>;
  /** Stops the transcription in progress; the answer, if any, is ignored. */
  cancelEngine: () => void;
  session: VoiceSession;
  /** Runs `run` after `ms` and returns the function that cancels it. */
  schedule: (run: () => void, ms: number) => () => void;
  /** The longest recording, in seconds. */
  maxSeconds: () => number;
  /** Whether the engine, the model and the microphone are there (the `unavailable` phase if not). */
  ready: () => boolean;
  /** Whether the button cannot be used right now: no documents folder, a question being answered,
   * the microphone in the hands of the notes. */
  blocked: () => boolean;
  getDraft: () => string;
  setDraft: (text: string) => void;
}

export interface Dictation {
  getSnapshot: () => DictationSnapshot;
  subscribe: (listener: () => void) => () => void;
  /** The input level (0 to 1) while recording, on its own channel so only the button redraws. */
  getLevel: () => number;
  subscribeLevel: (listener: () => void) => () => void;
  /** Starts a recording; `selection` is where the caret and the selection are in the box now. */
  start: (selection: { start: number; end: number }) => void;
  /** Ends the recording and sends it to the engine. */
  stop: () => void;
  /** Ends whatever is going on, at any stage, and discards what was recorded. */
  cancel: () => void;
  /** Puts the box back as it was before the last insertion. */
  undo: () => void;
}

/** The phase the button shows: `unavailable` whenever the engine or the microphone is not there and
 * nothing is going on (a recording in progress is never hidden by a status that changed under it). */
export function shownPhase(phase: DictationPhase, ready: boolean): DictationPhase {
  const quiet = phase === "idle" || phase === "done" || phase === "error";
  return quiet && !ready ? "unavailable" : phase;
}

const BUSY_PHASES: ReadonlySet<DictationPhase> = new Set<DictationPhase>([
  "requesting-permission",
  "recording",
  "transcribing",
]);

/** Escape ends a dictation in progress, but not while a dialog is open (it closes the dialog) and
 * not when something else already used the key (the list of names closes with it). */
export function escapeCancelsDictation(
  key: string,
  phase: DictationPhase,
  dialogOpen: boolean,
  alreadyHandled: boolean,
): boolean {
  return key === "Escape" && BUSY_PHASES.has(phase) && !dialogOpen && !alreadyHandled;
}

const IDLE: DictationSnapshot = { phase: "idle", error: null, result: null, caret: null };

export function createDictation(deps: DictationDeps): Dictation {
  let snapshot: DictationSnapshot = IDLE;
  let level = 0;
  /** Bumped by every start and every cancel: an answer that comes back for an older number is dropped. */
  let run = 0;
  let nonce = 0;
  let capture: AudioCapture | null = null;
  let remembered: RememberedSelection | null = null;
  let cancelTimer: (() => void) | null = null;
  const listeners = new Set<() => void>();
  const levelListeners = new Set<() => void>();

  const publish = (next: DictationSnapshot) => {
    snapshot = next;
    listeners.forEach((listener) => listener());
  };
  const setLevel = (value: number) => {
    level = value;
    levelListeners.forEach((listener) => listener());
  };
  const caretAt = (position: number, text: string) => ({ position, text, nonce: (nonce += 1) });

  /** Leaves every resource: the timer, the recorder, the level and the voice session. */
  const settle = () => {
    cancelTimer?.();
    cancelTimer = null;
    capture?.cancel();
    capture = null;
    remembered = null;
    deps.session.release("dictation");
    if (level !== 0) {
      setLevel(0);
    }
  };

  const fail = (key: string, values: Record<string, string | number> = {}) => {
    settle();
    publish({ phase: "error", error: { key, values }, result: null, caret: null });
  };

  const busy = () => BUSY_PHASES.has(snapshot.phase);

  const cancel = () => {
    const wasTranscribing = snapshot.phase === "transcribing";
    run += 1;
    settle();
    if (wasTranscribing) {
      deps.cancelEngine();
    }
    if (snapshot.phase !== "idle") {
      publish(IDLE);
    }
  };

  const stop = () => {
    if (snapshot.phase !== "recording" || capture === null) {
      return;
    }
    const mine = run;
    const recorder = capture;
    cancelTimer?.();
    cancelTimer = null;
    publish({ ...IDLE, phase: "transcribing" });
    void finish(mine, recorder);
  };

  const finish = async (mine: number, recorder: AudioCapture) => {
    let clip;
    try {
      clip = await recorder.stop();
    } catch {
      if (mine === run) {
        fail(captureFailureKey("failed"));
      }
      return;
    }
    if (mine !== run) {
      return;
    }
    // The recorder is spent; the rest is the engine's. A cancel from here on stops the engine.
    capture = null;
    setLevel(0);
    if (clip.durationSeconds < MIN_DICTATION_SECONDS) {
      fail("errors.speech_audio_too_short");
      return;
    }
    if (clip.stats.peak < QUIET_PEAK_FLOOR) {
      fail("errors.speech_audio_too_quiet");
      return;
    }
    const clipped = clip.stats.clippedShare > CLIPPED_SHARE_WARNING;
    let transcript: SpeechTranscript;
    try {
      transcript = await deps.transcribe(clip.wav);
    } catch (raw) {
      if (mine !== run) {
        return;
      }
      const error = normaliseError(raw);
      if (error.code === "speech_cancelled") {
        cancel();
        return;
      }
      fail(`errors.${error.code}`, error.data);
      return;
    }
    if (mine !== run) {
      return;
    }
    const insertion = insertTranscript(deps.getDraft(), transcript.text, remembered);
    if (insertion === null) {
      fail("speech.dictation.nothingHeard");
      return;
    }
    deps.setDraft(insertion.text);
    const result: DictationResult = {
      insertion,
      numbers: countNumbers(transcript.text),
      clipped,
      engineId: transcript.engineId,
      caseReliable: transcript.caseReliable,
    };
    const caret = caretAt(insertion.caret, insertion.text);
    settle();
    publish({ phase: "done", error: null, result, caret });
  };

  const start = (selection: { start: number; end: number }) => {
    if (busy() || !deps.ready() || deps.blocked()) {
      return;
    }
    const granted = deps.session.claim("dictation", cancel);
    if (!granted.granted) {
      fail(granted.reasonKey);
      return;
    }
    const mine = (run += 1);
    remembered = rememberSelection(deps.getDraft(), selection.start, selection.end);
    const recorder = deps.capture();
    capture = recorder;
    publish({ ...IDLE, phase: "requesting-permission" });
    recorder.start({ onLevel: (value) => mine === run && setLevel(value) }).then(
      () => {
        if (mine !== run) {
          recorder.cancel();
          return;
        }
        publish({ ...IDLE, phase: "recording" });
        const seconds = Math.max(1, deps.maxSeconds());
        cancelTimer = deps.schedule(stop, Math.max(0, seconds * 1_000 - CAP_MARGIN_MS));
      },
      (raw: unknown) => {
        if (mine === run) {
          fail(captureFailureKey(classifyCaptureError(raw)));
        } else {
          recorder.cancel();
        }
      },
    );
  };

  const undo = () => {
    const result = snapshot.result;
    if (snapshot.phase !== "done" || result === null) {
      return;
    }
    if (canUndo(deps.getDraft(), result.insertion)) {
      deps.setDraft(result.insertion.before.text);
      publish({
        ...IDLE,
        caret: caretAt(result.insertion.before.caret, result.insertion.before.text),
      });
    }
  };

  return {
    getSnapshot: () => snapshot,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getLevel: () => level,
    subscribeLevel: (listener) => {
      levelListeners.add(listener);
      return () => levelListeners.delete(listener);
    },
    start,
    stop,
    cancel,
    undo,
  };
}
