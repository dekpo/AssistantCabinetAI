import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { createWebviewCapture, microphoneSupport } from "../audio/recorder";
import { useTranslation } from "../i18n/I18nProvider";
import type { SpeechSettings } from "../lib/ipc";
import { useSpeechStatus } from "../state/useSpeechStatus";
import { featureAvailability } from "./availability";
import {
  createDictation,
  escapeCancelsDictation,
  shownPhase,
  type DictationNote,
  type DictationPhase,
  type DictationResult,
} from "./dictation";
import { canUndo } from "./insertion";
import { speechCancel, speechTranscribe } from "./ipc";
import { dialogIsOpen } from "./output/ReadAloudContext";
import { useVoiceSession } from "./VoiceSessionContext";

/**
 * What the question box needs to offer the microphone. It is handed to the composer as one optional
 * property: with none, the composer draws exactly what it drew before the voice existed.
 */
export interface DictationBridge {
  phase: DictationPhase;
  error: DictationNote | null;
  /** The last insertion, while the box still holds it (so the Undo and the notes under it are true). */
  result: DictationResult | null;
  /** A caret position to restore in the box once it holds `text`. */
  caret: { position: number; nonce: number; text: string } | null;
  /** Whether the button may be pressed. */
  disabled: boolean;
  /** The catalogue key of the sentence that says why the voice cannot be used (`unavailable`, or the
   * microphone is in use elsewhere), or null. */
  reasonKey: string | null;
  /** An analysis is running: dictation works, but the machine is busy. */
  analysing: boolean;
  level: { get: () => number; subscribe: (listener: () => void) => () => void };
  start: (selection: { start: number; end: number }) => void;
  stop: () => void;
  cancel: () => void;
  undo: () => void;
}

interface Inputs {
  speech: SpeechSettings;
  draft: string;
  setDraft: (draft: string) => void;
  /** A question is being answered. */
  chatBusy: boolean;
  /** No documents folder is chosen, so the box is locked. */
  needsFolder: boolean;
  /** An Analyse pass is running. */
  analysing: boolean;
}

function isUnderway(phase: DictationPhase): boolean {
  return phase === "requesting-permission" || phase === "recording" || phase === "transcribing";
}

function inBrowserTimer(run: () => void, ms: number): () => void {
  const id = setTimeout(run, ms);
  return () => clearTimeout(id);
}

/**
 * The dictation of the window, for the chat panel (which owns the draft, `send`, `stop` and `clear`).
 * It asks Rust one cheap question (what dictation can do) and starts nothing: the microphone opens
 * only when the button is pressed. Returns `undefined` while the voice is off or not yet known, so
 * the composer draws no microphone at all.
 */
export function useDictation(inputs: Inputs): DictationBridge | undefined {
  const { locale } = useTranslation();
  const { status } = useSpeechStatus(JSON.stringify([inputs.speech, locale]));
  const { session, holder } = useVoiceSession();
  const availability = featureAvailability(status, "dictation", microphoneSupport());
  const ready = availability.state === "ready";
  const sessionBusy = holder === "notes";
  const blocked = inputs.needsFolder || inputs.chatBusy || sessionBusy;

  const latest = useRef({ ...inputs, ready, blocked });
  latest.current = { ...inputs, ready, blocked };
  const [dictation] = useState(() =>
    createDictation({
      capture: createWebviewCapture,
      transcribe: (wav) => speechTranscribe(wav, "dictation"),
      cancelEngine: () => {
        void speechCancel().catch(() => undefined);
      },
      session,
      schedule: inBrowserTimer,
      maxSeconds: () => latest.current.speech.maxDictationSeconds,
      ready: () => latest.current.ready,
      blocked: () => latest.current.blocked,
      getDraft: () => latest.current.draft,
      setDraft: (draft) => latest.current.setDraft(draft),
    }),
  );
  const snapshot = useSyncExternalStore(dictation.subscribe, dictation.getSnapshot);
  const result =
    snapshot.result !== null && canUndo(inputs.draft, snapshot.result.insertion)
      ? snapshot.result
      : null;
  /* A finished dictation whose text was edited since has nothing left to say: the box is quiet again. */
  const phase = shownPhase(snapshot.phase === "done" && result === null ? "idle" : snapshot.phase, ready);

  useEffect(() => () => dictation.cancel(), [dictation]);

  /* A question starts being answered (sent by any path), or the voice is switched off: whatever is
     being recorded or transcribed ends. */
  const stopsHere = inputs.chatBusy || availability.state === "off";
  useEffect(() => {
    if (stopsHere) {
      dictation.cancel();
    }
  }, [stopsHere, dictation]);

  useEffect(() => {
    if (snapshot.phase === "idle" || snapshot.phase === "done" || snapshot.phase === "error") {
      return undefined;
    }
    const onKey = (event: KeyboardEvent) => {
      if (escapeCancelsDictation(event.key, snapshot.phase, dialogIsOpen(), event.defaultPrevented)) {
        dictation.cancel();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [snapshot.phase, dictation]);

  const level = useMemo(
    () => ({ get: dictation.getLevel, subscribe: dictation.subscribeLevel }),
    [dictation],
  );

  if (availability.state === "off" || availability.state === "unknown") {
    return undefined;
  }
  const reasonKey =
    availability.state === "blocked"
      ? availability.reasonKey
      : sessionBusy
        ? "speech.session.microphoneBusy"
        : null;
  return {
    phase,
    error: snapshot.error,
    result,
    caret: snapshot.caret,
    disabled: phase === "unavailable" || (blocked && !isUnderway(phase)),
    reasonKey,
    analysing: inputs.analysing,
    level,
    start: dictation.start,
    stop: dictation.stop,
    cancel: dictation.cancel,
    undo: dictation.undo,
  };
}
