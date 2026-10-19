import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { clipFromSamples, createWebviewCapture, microphoneSupport } from "../audio/recorder";
import type { SpeechSettings } from "../lib/ipc";
import { activeDraft, hasContent } from "../lib/notesDrafts";
import type { NotesSession } from "../lib/notesSession";
import { useSpeechStatus } from "../state/useSpeechStatus";
import { featureAvailability } from "./availability";
import type { DictationNote } from "./dictation";
import { speechApplyDictationCommands, speechCancel, speechTranscribe } from "./ipc";
import {
  createLongDictation,
  escapeCancelsLong,
  isActive,
  type Delivery,
  type LongPhase,
} from "./longDictation";
import { appendToNote } from "./noteText";
import { dialogIsOpen } from "./output/ReadAloudContext";
import type { RepairSource } from "./repairChips";
import { useVoiceSession } from "./VoiceSessionContext";

/**
 * What the Notes section needs to offer the pill: one optional bundle. `level` is a channel of its
 * own, so the input level moves the waveform without redrawing the window.
 */
export interface LongDictationBridge {
  phase: LongPhase;
  /** A recording or its transcription is going on. */
  active: boolean;
  /** The microphone is open (or being asked for): the pill shows the waveform. */
  recording: boolean;
  /** Passages cut and not yet put in the note. */
  waiting: number;
  /** Positions of the passages that could not be transcribed. */
  failed: number[];
  error: DictationNote | null;
  capReached: boolean;
  startedAt: number | null;
  /** The pill cannot be pressed now. */
  disabled: boolean;
  /** The catalogue key of the sentence that says why the voice cannot be used, or null. */
  reasonKey: string | null;
  level: { get: () => number; subscribe: (listener: () => void) => () => void };
  /** The press of the pill: starts, finishes (the queue is still emptied) or abandons the permission. */
  toggle: () => void;
  /** Ends everything and drops the passages that were not transcribed. */
  cancel: () => void;
  dismiss: () => void;
  /** The words of the last passage that went into the active note, for the name suggestions. */
  heard: RepairSource | null;
}

interface Inputs {
  /** `null` until the settings are read. */
  speech: SpeechSettings | null;
  /** The interface locale: `App` sits outside the language provider, so it is handed in. */
  locale: string;
  session: NotesSession;
  /** The Notes section exists (its own switch). */
  notesEnabled: boolean;
  /** A save is under way: the notes cannot change. */
  saving: boolean;
}

/** Only used before the settings are read, when no recording can have started. */
const DEFAULT_NOTE_SECONDS = 1_800;

function inBrowserTimer(run: () => void, ms: number): () => void {
  const id = setTimeout(run, ms);
  return () => clearTimeout(id);
}

/**
 * The long dictation of the notes. It asks Rust one cheap question (what dictation can do) and
 * starts nothing: the microphone opens only when the pill is pressed. Returns `undefined` while the
 * voice is off or not yet known, so no pill is drawn at all.
 */
export function useLongDictation(inputs: Inputs): LongDictationBridge | undefined {
  const { status } = useSpeechStatus(JSON.stringify([inputs.speech, inputs.locale]));
  const { session: voice, holder } = useVoiceSession();
  const availability = featureAvailability(status, "dictation", microphoneSupport());
  const ready = availability.state === "ready" && inputs.notesEnabled && inputs.speech !== null;
  const sessionBusy = holder === "dictation";
  const blocked = inputs.saving || sessionBusy;
  const [heard, setHeard] = useState<RepairSource | null>(null);

  const latest = useRef({ ...inputs, ready, blocked });
  latest.current = { ...inputs, ready, blocked };

  const deliver = useCallback(({ pieces, caseReliable }: Delivery) => {
    const notes = latest.current.session;
    let last: RepairSource | null = null;
    for (const [index, piece] of pieces.entries()) {
      /* A new note is opened only after one that holds something: no empty note is left behind. */
      if (index > 0 && hasContent(activeDraft(notes.getSnapshot().notes))) {
        notes.add();
      }
      const active = activeDraft(notes.getSnapshot().notes);
      const appended = appendToNote(active.text, piece);
      if (appended.text !== active.text) {
        notes.edit(active.id, { text: appended.text });
        last = {
          id: {},
          words: appended.words,
          start: appended.start,
          draft: appended.text,
          caseReliable,
        };
      }
    }
    setHeard(last);
  }, []);

  const [dictation] = useState(() =>
    createLongDictation({
      capture: createWebviewCapture,
      clip: clipFromSamples,
      transcribe: (wav) => speechTranscribe(wav, "note"),
      cancelEngine: () => {
        void speechCancel().catch(() => undefined);
      },
      applyCommands: speechApplyDictationCommands,
      deliver: (delivery) => deliver(delivery),
      session: voice,
      schedule: inBrowserTimer,
      now: Date.now,
      maxSeconds: () => latest.current.speech?.maxNoteSeconds ?? DEFAULT_NOTE_SECONDS,
      ready: () => latest.current.ready,
      blocked: () => latest.current.blocked,
    }),
  );
  const snapshot = useSyncExternalStore(dictation.subscribe, dictation.getSnapshot);

  useEffect(() => () => dictation.cancel(), [dictation]);

  /* The voice or the Notes section is switched off: whatever is going on ends. */
  const stopsHere = availability.state === "off" || !inputs.notesEnabled;
  useEffect(() => {
    if (stopsHere) {
      dictation.cancel();
    }
  }, [stopsHere, dictation]);

  const active = isActive(snapshot.phase);
  useEffect(() => {
    if (!active) {
      return undefined;
    }
    const onKey = (event: KeyboardEvent) => {
      if (escapeCancelsLong(event.key, snapshot.phase, dialogIsOpen(), event.defaultPrevented)) {
        dictation.cancel();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [active, snapshot.phase, dictation]);

  const level = useMemo(
    () => ({ get: dictation.getLevel, subscribe: dictation.subscribeLevel }),
    [dictation],
  );
  const toggle = useCallback(() => {
    const phase = dictation.getSnapshot().phase;
    if (phase === "idle") {
      dictation.start();
    } else if (phase === "recording") {
      dictation.stop();
    } else if (phase === "requesting-permission") {
      dictation.cancel();
    }
  }, [dictation]);

  if (
    availability.state === "off" ||
    availability.state === "unknown" ||
    !inputs.notesEnabled ||
    inputs.speech === null
  ) {
    return undefined;
  }
  const reasonKey =
    availability.state === "blocked"
      ? availability.reasonKey
      : sessionBusy
        ? "speech.session.microphoneBusy"
        : null;
  return {
    phase: snapshot.phase,
    active,
    recording: snapshot.phase === "recording" || snapshot.phase === "requesting-permission",
    waiting: snapshot.waiting,
    failed: snapshot.failed,
    error: snapshot.error,
    capReached: snapshot.capReached,
    startedAt: snapshot.startedAt,
    disabled: availability.state === "blocked" || (blocked && !active) || snapshot.phase === "finishing",
    reasonKey,
    level,
    toggle,
    cancel: dictation.cancel,
    dismiss: dictation.dismiss,
    heard,
  };
}
