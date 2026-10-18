import { Channel, invoke } from "@tauri-apps/api/core";

/**
 * The speech commands of Rust: `speech_status`, `notes_preview`, `notes_save`, `speech_transcribe`,
 * `speech_cancel` and the three of the model (`speech_model_install`, `speech_model_verify` and
 * `speech_model_cancel`). The audio goes to Rust as a raw body, and the language and the cap on the
 * length always come from Rust (settings and language table), never from the webview.
 */

/** The speech-to-text model on disk. `unverified`: every file is there but none was checked yet (a
 * model placed by hand); Verify checks it. */
export type ModelState = "missing" | "unverified" | "present" | "corrupt";

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

/** The extension of a saved note. Markdown unless the user chose plain text. */
export type NoteFormat = "md" | "txt";

/** One note as the user reviewed it. Never a path: Rust derives the folder and the file name. */
export interface NoteDraft {
  /** What the user typed for the name; may be empty (the file is then named by its time alone). */
  name: string;
  text: string;
  format?: NoteFormat;
}

/** A note file as it would be written (preview) or was written (save). */
export interface NoteFile {
  /** Relative to the documents folder, forward-slash separated: the key the index uses. */
  relativePath: string;
  fileName: string;
  bytes: number;
  chars: number;
  lines: number;
}

/**
 * The files a save of these notes would write, with the names they would get. Writes nothing.
 * Refusals are machine codes: `notes_folder_unavailable`, `notes_name_invalid`, `notes_too_large`,
 * `notes_too_many`.
 */
export function notesPreview(drafts: NoteDraft[]): Promise<NoteFile[]> {
  return invoke<NoteFile[]>("notes_preview", { drafts });
}

/**
 * Writes the reviewed notes as new files, all or none. It does not analyse them: the caller runs
 * the Analyse pass afterwards, and a failure of that pass is `notes_analysis_failed`, never an
 * undo of the save.
 */
export function notesSave(drafts: NoteDraft[]): Promise<NoteFile[]> {
  return invoke<NoteFile[]>("notes_save", { drafts });
}

/** What a clip is for. It decides the cap on its length, which Rust reads from the settings. */
export type SpeechPurpose = "dictation" | "note";

/** What the engine heard, and how long it took. The text only fills a text box. */
export interface SpeechTranscript {
  text: string;
  audioMs: number;
  processingMs: number;
  engineId: string;
}

/**
 * Transcribes one clip: `wav` is the file `encodeWav` wrote and nothing else (a raw request body,
 * not JSON); the purpose travels as a header. Refusals are machine codes: `speech_unavailable`,
 * `speech_locale_unsupported`, `speech_engine_missing`, `speech_model_missing`,
 * `speech_model_corrupt`, `speech_busy`, `speech_audio_invalid`, `speech_audio_too_short`,
 * `speech_audio_too_quiet`, `speech_audio_too_long`, `speech_cancelled`, `speech_timeout`,
 * `speech_engine_failed`, `speech_output_rejected`.
 */
export function speechTranscribe(
  wav: Uint8Array,
  purpose: SpeechPurpose = "dictation",
): Promise<SpeechTranscript> {
  return invoke<SpeechTranscript>("speech_transcribe", wav, { headers: { purpose } });
}

/** Stops the transcription in progress, if any. Resolves to whether there was one. */
export function speechCancel(): Promise<boolean> {
  return invoke<boolean>("speech_cancel");
}

/** How far the install of the model has got. `download` runs first, then `unpack`. */
export interface ModelProgress {
  phase: "download" | "unpack";
  doneBytes: number;
  totalBytes: number;
}

/**
 * Downloads and installs the speech model. Only a button calls it. Progress arrives on the channel;
 * refusals are machine codes: `speech_model_download_failed` (its data carries the folder where the
 * files can be placed by hand), `speech_model_corrupt`, `speech_cancelled`, `speech_busy`.
 */
export function speechModelInstall(onProgress: (progress: ModelProgress) => void): Promise<void> {
  const channel = new Channel<ModelProgress>();
  channel.onmessage = onProgress;
  return invoke<void>("speech_model_install", { onProgress: channel });
}

/** Checks the files in the model folder against the pinned digests; this is how a model placed by
 * hand becomes usable. Refusals: `speech_model_missing`, `speech_model_corrupt`, `speech_cancelled`. */
export function speechModelVerify(): Promise<void> {
  return invoke<void>("speech_model_verify");
}

/** Stops the install or the verify in progress, if any. Resolves to whether there was one. */
export function speechModelCancel(): Promise<boolean> {
  return invoke<boolean>("speech_model_cancel");
}
