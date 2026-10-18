import type { SpeechStatus } from "../ipc";

/**
 * Which voice reads, and whether the button is drawn at all. Pure functions of what Rust answered
 * (`speech_status`) and of the engine setting, so the matrix is tested without a window.
 *
 * - `piper`: the local worker, when Rust found a voice for the language (its probe).
 * - `system`: the operating system's local voices, only when the user chose them in the settings.
 *   Rust cannot probe the window's voices, and a system voice may be an online one, so it is never
 *   picked for the user: choosing it is the step that follows the offline proof.
 * - `none`: nothing may read; the button then answers with `tts_no_local_voice`.
 */
export type EngineChoice = "piper" | "system" | "none";

export type TtsEngineSetting = "piper" | "system" | null;

/** The codes of `speech_status.features.readAloud.reason` after which no button is drawn: the voice
 * or the read-aloud switch is off (a choice, not a problem), or the language has no read-aloud. */
const NO_BUTTON_REASONS: ReadonlySet<string> = new Set(["speech_unavailable", "speech_locale_unsupported"]);

export function chooseEngine(status: SpeechStatus | null, setting: TtsEngineSetting): EngineChoice {
  if (setting === "system") {
    return "system";
  }
  if (status?.features.readAloud.available === true) {
    return "piper";
  }
  return "none";
}

export interface ReadAloudVisibility {
  visible: boolean;
  engine: EngineChoice;
}

/**
 * Whether the button is drawn under a finished answer. It agrees with `speech_status.readAloud`:
 * hidden until Rust has answered, with the voice master switch off, with the read-aloud switch off
 * and in a language without read-aloud; drawn otherwise, even when no voice is installed, because
 * pressing it then tells why (`tts_no_local_voice`) instead of leaving the user to guess.
 */
export function readAloudVisibility(
  status: SpeechStatus | null,
  setting: TtsEngineSetting,
): ReadAloudVisibility {
  const hidden: ReadAloudVisibility = { visible: false, engine: "none" };
  if (status === null || !status.enabled) {
    return hidden;
  }
  const { available, reason } = status.features.readAloud;
  if (!available && reason !== null && NO_BUTTON_REASONS.has(reason)) {
    return hidden;
  }
  return { visible: true, engine: chooseEngine(status, setting) };
}
