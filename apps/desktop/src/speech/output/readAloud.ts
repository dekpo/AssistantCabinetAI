import type { TranslationValues } from "../../i18n/translate";
import type { ReadingItem, ReadingNoteCode, ReadingPart } from "../ipc";
import type { VoiceSession } from "../voiceSession";
import { outputErrorCode, type ReadAloudEngine, type SpeakProgress } from "./engine";
import { spokenReading } from "./pauses";

/**
 * The one read-aloud of the window. It holds who is being read, prepares the text, takes the voice
 * session, and hands the segments to the engine. One voice at a time: starting a reading ends the one
 * in progress, whichever answer it was for. Nothing here starts by itself: only `start`, which a click
 * calls, reaches the engine. Pure (no React, no Tauri): the bridge, the engine and the session are
 * handed in.
 */

export type ReadAloudPhase = "idle" | "preparing" | "speaking";

/** A sentence to show beside the button of the answer it concerns, as a catalogue key. */
export interface ReadAloudProblem {
  entryId: string;
  key: string;
  values: TranslationValues;
}

export interface ReadAloudState {
  /** The answer being prepared or read, or null. */
  entryId: string | null;
  phase: ReadAloudPhase;
  progress: SpeakProgress | null;
  problem: ReadAloudProblem | null;
}

export interface ReadAloudDeps {
  prepare: (parts: ReadingPart[], locale: string) => Promise<ReadingItem[]>;
  /** The engine the settings choose right now, or null when none may read. */
  engine: () => ReadAloudEngine | null;
  session: VoiceSession;
  options: () => { rate: number; voiceId: string | null };
  noteText: (code: ReadingNoteCode, count: number) => string;
}

export interface ReadAloud {
  getSnapshot: () => ReadAloudState;
  subscribe: (listener: () => void) => () => void;
  /** Reads an answer aloud. Only a click on its button calls this. */
  start: (entryId: string, parts: ReadingPart[], locale: string) => Promise<void>;
  /** Ends whatever is being read. */
  stop: () => void;
  /** Ends the reading of this answer, and only of this answer (its button is leaving the screen). */
  stopFor: (entryId: string) => void;
  /** Forgets the sentence shown for this answer. */
  dismissProblem: (entryId: string) => void;
}

const IDLE: ReadAloudState = { entryId: null, phase: "idle", progress: null, problem: null };

/** The key of the sentence for a reading with nothing to say. */
export const NOTHING_TO_READ_KEY = "speech.readAloud.nothingToRead";

export function createReadAloud(deps: ReadAloudDeps): ReadAloud {
  let state = IDLE;
  let generation = 0;
  const listeners = new Set<() => void>();
  const set = (next: ReadAloudState) => {
    state = next;
    listeners.forEach((listener) => listener());
  };

  const endCurrent = () => {
    generation += 1;
    deps.engine()?.stop();
    deps.session.release("readAloud");
  };

  const stop = () => {
    endCurrent();
    if (state.entryId !== null || state.phase !== "idle") {
      set({ ...IDLE, problem: state.problem });
    }
  };

  const fail = (entryId: string, key: string, values: TranslationValues = {}) => {
    deps.session.release("readAloud");
    set({ entryId: null, phase: "idle", progress: null, problem: { entryId, key, values } });
  };

  return {
    getSnapshot: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    start: async (entryId, parts, locale) => {
      endCurrent();
      const mine = generation;
      const failure = (code: string) => fail(entryId, `errors.${code}`, { locale });
      set({ entryId, phase: "preparing", progress: null, problem: null });

      const engine = deps.engine();
      if (engine === null) {
        failure("tts_no_local_voice");
        return;
      }
      const claim = deps.session.claim("readAloud", stop);
      if (!claim.granted) {
        fail(entryId, claim.reasonKey);
        return;
      }

      let items: ReadingItem[];
      try {
        items = await deps.prepare(parts, locale);
      } catch (error) {
        if (mine === generation) {
          failure(outputErrorCode(error));
        }
        return;
      }
      if (mine !== generation) {
        return;
      }
      const reading = spokenReading(items, deps.noteText);
      if (!reading.hasSpeech) {
        fail(entryId, NOTHING_TO_READ_KEY);
        return;
      }

      set({ entryId, phase: "speaking", progress: { done: 0, total: reading.segments.length }, problem: null });
      try {
        await engine.speak(reading.segments, {
          locale,
          ...deps.options(),
          onProgress: (progress) => {
            if (mine === generation) {
              set({ entryId, phase: "speaking", progress, problem: null });
            }
          },
        });
      } catch (error) {
        if (mine === generation) {
          failure(outputErrorCode(error));
        }
        return;
      }
      if (mine === generation) {
        deps.session.release("readAloud");
        set(IDLE);
      }
    },
    stop,
    stopFor: (entryId) => {
      if (state.entryId === entryId) {
        stop();
      }
    },
    dismissProblem: (entryId) => {
      if (state.problem?.entryId === entryId) {
        set({ ...state, problem: null });
      }
    },
  };
}

/** What a button does when it leaves the screen (a new question, a regenerate, a cleared
 * conversation all hide the row it sits in): its own reading ends, and so does its sentence. */
export function leaveEntry(readAloud: ReadAloud, entryId: string): void {
  readAloud.stopFor(entryId);
  readAloud.dismissProblem(entryId);
}

/** Whether Escape ends the reading: only when something is being read and no dialog is open (the
 * dialog's own Escape closes the dialog and leaves the voice alone). */
export function escapeStopsReading(key: string, phase: ReadAloudPhase, dialogOpen: boolean): boolean {
  return key === "Escape" && phase !== "idle" && !dialogOpen;
}
