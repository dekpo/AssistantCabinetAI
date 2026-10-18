import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import type { SpeechSettings } from "../../lib/ipc";
import { useSpeechStatus } from "../../state/useSpeechStatus";
import { useVoiceSession } from "../VoiceSessionContext";
import { speechPrepareReading } from "../ipc";
import { readAloudVisibility, type EngineChoice } from "./engineChoice";
import type { ReadAloudEngine } from "./engine";
import { noteSentence } from "./pauses";
import { createReadAloud, escapeStopsReading, type ReadAloud, type ReadAloudState } from "./readAloud";
import { browserSystemVoiceEngine } from "./systemVoices";

/** What a button under an answer needs: the one reading, who is being read, and whether to be drawn. */
export interface ReadAloudValue {
  controller: ReadAloud;
  state: ReadAloudState;
  visible: boolean;
}

export const ReadAloudContext = createContext<ReadAloudValue | null>(null);

/**
 * The engine for a choice. The Piper worker arrives with its own phase; until it does a Piper choice
 * has no engine, so a reading is refused with `tts_no_local_voice` and never falls back to an
 * operating-system voice the user did not choose.
 */
function engineFor(choice: EngineChoice, cache: { system?: ReadAloudEngine | null }): ReadAloudEngine | null {
  if (choice === "system") {
    cache.system ??= browserSystemVoiceEngine();
    return cache.system;
  }
  return null;
}

/** Whether a modal dialog is on screen. Its own Escape closes it; the voice is left alone. */
export function dialogIsOpen(): boolean {
  return document.querySelector('[aria-modal="true"], [role="dialog"], [role="alertdialog"]') !== null;
}

/**
 * Mounted once, inside the language provider and the voice session. It asks Rust one cheap question
 * (what read-aloud can do), builds the one reading of the window and ends it on Escape, on a window
 * close and when read-aloud is switched off. It draws nothing and starts no speech.
 */
export function ReadAloudProvider({ speech, children }: { speech: SpeechSettings; children: ReactNode }) {
  const { t, locale } = useTranslation();
  const { session } = useVoiceSession();
  const { status } = useSpeechStatus(JSON.stringify([speech, locale]));
  const { visible, engine: choice } = readAloudVisibility(status, speech.ttsEngine);

  const latest = useRef({ speech, choice, t });
  latest.current = { speech, choice, t };
  const cache = useRef<{ system?: ReadAloudEngine | null }>({});
  const [controller] = useState(() =>
    createReadAloud({
      prepare: speechPrepareReading,
      engine: () => engineFor(latest.current.choice, cache.current),
      session,
      options: () => ({ rate: latest.current.speech.ttsRate, voiceId: latest.current.speech.ttsVoice }),
      noteText: (code, count) => noteSentence(latest.current.t, code, count),
    }),
  );
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (escapeStopsReading(event.key, controller.getSnapshot().phase, dialogIsOpen())) {
        controller.stop();
      }
    };
    const onClose = () => controller.stop();
    document.addEventListener("keydown", onKey);
    window.addEventListener("beforeunload", onClose);
    return () => {
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("beforeunload", onClose);
      controller.stop();
    };
  }, [controller]);

  /* The voice or the read-aloud switch turned off while something is read: it stops. */
  useEffect(() => {
    if (!visible) {
      controller.stop();
    }
  }, [visible, controller]);

  const value = useMemo(() => ({ controller, state, visible }), [controller, state, visible]);
  return <ReadAloudContext.Provider value={value}>{children}</ReadAloudContext.Provider>;
}

/** The reading of the window, or null outside the provider (the button then draws nothing). */
export function useReadAloud(): ReadAloudValue | null {
  return useContext(ReadAloudContext);
}
