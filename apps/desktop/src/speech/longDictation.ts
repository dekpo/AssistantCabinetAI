import {
  captureFailureKey,
  classifyCaptureError,
  type AudioCapture,
  type CaptureResult,
} from "../audio/recorder";
import { createSegmenter, type Segment, type Segmenter } from "../audio/segmenter";
import { normaliseError } from "../lib/errors";
import {
  CAP_MARGIN_MS,
  MIN_DICTATION_SECONDS,
  QUIET_PEAK_FLOOR,
  type DictationNote,
} from "./dictation";
import type { AppliedCommands, SpeechTranscript } from "./ipc";
import type { VoiceSession } from "./voiceSession";

/**
 * The long dictation of a note, as a state machine with no React and no Tauri. The microphone is
 * read as a stream; the segmenter cuts it into passages while the user goes on speaking; the passages
 * are transcribed one at a time, in order, and each text is handed to the note as soon as it is
 * ready. Audio is never kept: a passage is dropped from memory the moment its text (or its failure)
 * is known, and nothing here writes, sends or stores anything. A transcript only fills the note.
 */

export type LongPhase = "idle" | "requesting-permission" | "recording" | "finishing";

export interface LongSnapshot {
  phase: LongPhase;
  /** Passages cut and not yet transcribed, the one being transcribed included. */
  waiting: number;
  /** Positions (from 1, in the order the passages were cut) of the passages that could not be
   * transcribed. Their audio is already dropped. Cleared by the next recording. */
  failed: number[];
  /** Why the recording could not start, or the first reason a passage failed. */
  error: DictationNote | null;
  /** The recording stopped by itself at the length cap. */
  capReached: boolean;
  /** When the recording started (`now()`), while it runs. */
  startedAt: number | null;
}

/** What goes into the notes for one passage. */
export interface Delivery {
  pieces: string[];
  caseReliable: boolean;
}

export interface LongDictationDeps {
  /** A new recorder for each recording. */
  capture: () => AudioCapture;
  /** One passage as a WAV clip. */
  clip: (samples: Float32Array, rate: number) => Promise<CaptureResult>;
  /** Sends the WAV to the engine as a note. Rejections are machine codes. */
  transcribe: (wav: Uint8Array) => Promise<SpeechTranscript>;
  cancelEngine: () => void;
  /** Applies the voice commands of the language (a no-op when the setting is off). */
  applyCommands: (text: string, pendingHeading: number) => Promise<AppliedCommands>;
  /** Puts the text into the notes. Called once per passage, in order. */
  deliver: (delivery: Delivery) => void;
  session: VoiceSession;
  schedule: (run: () => void, ms: number) => () => void;
  now: () => number;
  /** The longest recording, in seconds. */
  maxSeconds: () => number;
  ready: () => boolean;
  blocked: () => boolean;
}

export interface LongDictation {
  getSnapshot: () => LongSnapshot;
  subscribe: (listener: () => void) => () => void;
  /** The input level (0 to 1) while recording, on its own channel so only the pill redraws. */
  getLevel: () => number;
  subscribeLevel: (listener: () => void) => () => void;
  start: () => void;
  /** Ends the recording; the passages already cut are still transcribed and put in the note. */
  stop: () => void;
  /** Ends everything at any stage and drops the passages that were not transcribed. */
  cancel: () => void;
  /** Forgets the notice of the last recording (failed passages, the cap, an error). */
  dismiss: () => void;
}

/** A passage the engine finds too short or too quiet is silence, not a failure. */
const SILENT_CODES: ReadonlySet<string> = new Set(["speech_audio_too_short", "speech_audio_too_quiet"]);

const IDLE: LongSnapshot = {
  phase: "idle",
  waiting: 0,
  failed: [],
  error: null,
  capReached: false,
  startedAt: null,
};

/** Whether a recording or its transcription is going on (the pill is busy, a save must wait). */
export function isActive(phase: LongPhase): boolean {
  return phase !== "idle";
}

/** Escape ends the recording, but not while a dialog is open and not when the key was used already. */
export function escapeCancelsLong(
  key: string,
  phase: LongPhase,
  dialogOpen: boolean,
  alreadyHandled: boolean,
): boolean {
  return key === "Escape" && isActive(phase) && !dialogOpen && !alreadyHandled;
}

interface Waiting {
  position: number;
  segment: Segment;
}

export function createLongDictation(deps: LongDictationDeps): LongDictation {
  let snapshot: LongSnapshot = IDLE;
  let level = 0;
  /** Bumped by every start and every cancel: an answer for an older number is dropped. */
  let run = 0;
  let capture: AudioCapture | null = null;
  let segmenter: Segmenter | null = null;
  let cancelTimer: (() => void) | null = null;
  let queue: Waiting[] = [];
  let inFlight = false;
  let draining = false;
  let cut = 0;
  let pendingHeading = 0;
  /** The recording has ended: the queue is the only thing left. */
  let ended = false;
  const listeners = new Set<() => void>();
  const levelListeners = new Set<() => void>();

  const publish = (change: Partial<LongSnapshot>) => {
    snapshot = { ...snapshot, ...change };
    listeners.forEach((listener) => listener());
  };
  const setLevel = (value: number) => {
    level = value;
    levelListeners.forEach((listener) => listener());
  };
  const count = () => queue.length + (inFlight ? 1 : 0);

  /** Leaves every resource: the timer, the recorder, the level and the voice session. */
  const settle = () => {
    cancelTimer?.();
    cancelTimer = null;
    capture?.cancel();
    capture = null;
    segmenter = null;
    deps.session.release("notes");
    if (level !== 0) {
      setLevel(0);
    }
  };

  const complete = (mine: number) => {
    if (mine !== run || !ended || count() > 0 || draining) {
      return;
    }
    settle();
    publish({ phase: "idle", waiting: 0, startedAt: null });
  };

  const fail = (key: string, values: Record<string, string | number> = {}) => {
    run += 1;
    queue = [];
    inFlight = false;
    settle();
    publish({ ...IDLE, error: { key, values } });
  };

  const transcribeOne = async (mine: number, item: Waiting): Promise<void> => {
    let clip: CaptureResult;
    try {
      clip = await deps.clip(item.segment.samples, item.segment.rate);
    } catch {
      noteFailure(mine, item.position, { key: "errors.speech_audio_invalid", values: {} });
      return;
    }
    if (mine !== run) {
      return;
    }
    if (clip.durationSeconds < MIN_DICTATION_SECONDS || clip.stats.peak < QUIET_PEAK_FLOOR) {
      return;
    }
    let transcript: SpeechTranscript;
    try {
      transcript = await deps.transcribe(clip.wav);
    } catch (raw) {
      if (mine !== run) {
        return;
      }
      const error = normaliseError(raw);
      if (error.code === "speech_cancelled") {
        return;
      }
      if (!SILENT_CODES.has(error.code)) {
        noteFailure(mine, item.position, { key: `errors.${error.code}`, values: error.data });
      }
      return;
    }
    if (mine !== run) {
      return;
    }
    const text = transcript.text.trim();
    if (text === "") {
      return;
    }
    let applied: AppliedCommands;
    try {
      applied = await deps.applyCommands(text, pendingHeading);
    } catch {
      applied = { pieces: [text], pendingHeading: 0 };
    }
    if (mine !== run) {
      return;
    }
    pendingHeading = applied.pendingHeading;
    deps.deliver({ pieces: applied.pieces, caseReliable: transcript.caseReliable });
  };

  const noteFailure = (mine: number, position: number, note: DictationNote) => {
    if (mine !== run) {
      return;
    }
    publish({
      failed: [...snapshot.failed, position],
      error: snapshot.error ?? note,
    });
  };

  const drain = async (mine: number) => {
    if (draining) {
      return;
    }
    draining = true;
    while (mine === run && queue.length > 0) {
      const item = queue.shift() as Waiting;
      inFlight = true;
      publish({ waiting: count() });
      await transcribeOne(mine, item);
      if (mine !== run) {
        // A cancel (or a new recording) owns the flags now.
        return;
      }
      inFlight = false;
      publish({ waiting: count() });
    }
    draining = false;
    complete(mine);
  };

  const enqueue = (mine: number, segment: Segment) => {
    cut += 1;
    queue.push({ position: cut, segment });
    publish({ waiting: count() });
    void drain(mine);
  };

  const cancel = () => {
    if (snapshot.phase === "idle" && count() === 0 && capture === null) {
      return;
    }
    const wasTranscribing = inFlight;
    run += 1;
    queue = [];
    inFlight = false;
    draining = false;
    ended = false;
    settle();
    if (wasTranscribing) {
      deps.cancelEngine();
    }
    publish({ ...IDLE });
  };

  const stop = () => {
    if (snapshot.phase !== "recording" || capture === null) {
      return;
    }
    const mine = run;
    const recorder = capture;
    cancelTimer?.();
    cancelTimer = null;
    publish({ phase: "finishing", startedAt: null });
    void recorder.stop().then(
      () => finishRecording(mine),
      () => finishRecording(mine),
    );
  };

  const finishRecording = (mine: number) => {
    if (mine !== run) {
      return;
    }
    // The recorder is spent. Its last block has been handed over: cut what is left.
    capture = null;
    setLevel(0);
    segmenter?.finish();
    segmenter = null;
    ended = true;
    complete(mine);
  };

  const start = () => {
    if (snapshot.phase !== "idle" || !deps.ready() || deps.blocked()) {
      return;
    }
    const granted = deps.session.claim("notes", cancel);
    if (!granted.granted) {
      publish({ ...IDLE, error: { key: granted.reasonKey, values: {} } });
      return;
    }
    const mine = (run += 1);
    queue = [];
    inFlight = false;
    draining = false;
    ended = false;
    cut = 0;
    pendingHeading = 0;
    const recorder = deps.capture();
    capture = recorder;
    let current: Segmenter | null = null;
    publish({ ...IDLE, phase: "requesting-permission" });
    recorder
      .start({
        onLevel: (value) => mine === run && setLevel(value),
        onBlock: (block, rate) => {
          if (mine !== run) {
            return;
          }
          current ??= createSegmenter(rate, (segment) => enqueue(mine, segment));
          segmenter = current;
          current.push(block);
        },
      })
      .then(
        () => {
          if (mine !== run) {
            recorder.cancel();
            return;
          }
          publish({ phase: "recording", startedAt: deps.now() });
          const seconds = Math.max(1, deps.maxSeconds());
          cancelTimer = deps.schedule(() => {
            publish({ capReached: true });
            stop();
          }, Math.max(0, seconds * 1_000 - CAP_MARGIN_MS));
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
    dismiss: () => {
      if (snapshot.phase === "idle") {
        publish({ failed: [], error: null, capReached: false });
      }
    },
  };
}
