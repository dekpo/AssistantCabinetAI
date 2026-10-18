import { invoke } from "@tauri-apps/api/core";

/**
 * The speech commands of Rust. Only `speech_status` exists so far; the audio goes to Rust as a raw
 * body in a later phase, and the language always comes from Rust (settings and language table),
 * never from the webview.
 */

/** The speech-to-text model on disk. */
export type ModelState = "missing" | "present" | "corrupt";

/** Whether one feature can run; `reason` is null or the machine code of the first obstacle
 * (`speech_unavailable`, `speech_locale_unsupported`, `speech_engine_missing`,
 * `speech_model_missing`, `speech_model_corrupt`, `tts_no_local_voice`,
 * `notes_folder_unavailable`), which `errors.<code>` already says in words. */
export interface FeatureStatus {
  available: boolean;
  reason: string | null;
}

export type SpeechFeature = "dictation" | "readAloud" | "notes";

export type VoiceGender = "female" | "male" | "mixed";

export interface LanguageVoice {
  gender: VoiceGender;
  id: string;
}

/** One row of the language table (`speech/languages.rs`). */
export interface SpeechLanguage {
  id: string;
  support: "enabled" | "planned";
  dictation: boolean;
  readAloud: boolean;
  voices: LanguageVoice[];
}

export interface SpeechStatus {
  /** The voice master switch. */
  enabled: boolean;
  /** The locale speech works in: the override if set, else the interface locale. */
  locale: string;
  features: Record<SpeechFeature, FeatureStatus>;
  /** Shown whatever the switches say, so a missing or damaged model is not hidden by "voice is off". */
  sttModel: ModelState;
  languages: SpeechLanguage[];
}

/** What each speech feature can do right now. Reads settings and probes, starts nothing. */
export function speechStatus(): Promise<SpeechStatus> {
  return invoke<SpeechStatus>("speech_status");
}
