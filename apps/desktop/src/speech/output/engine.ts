/**
 * The read-aloud engine behind one interface, so the button, the queue and the voice session never
 * know whether the voice is the operating system's or a worker's. An engine speaks a list of
 * segments one after the other and can be stopped at any moment; it never decides what to say (that
 * is `speech_prepare_reading` in Rust) and never reaches a network.
 */

export type EngineId = "piper" | "system";

/** One piece of text to say, and how long the voice rests after it. */
export interface SpeakSegment {
  text: string;
  pauseAfterMs: number;
}

/** `done` segments have been spoken out of `total`. */
export interface SpeakProgress {
  done: number;
  total: number;
}

export interface SpeakOptions {
  /** The language the text is written in: the interface locale. */
  locale: string;
  /** Multiple of the natural rate (the `ttsRate` setting). */
  rate: number;
  /** The voice the user chose, or null for the first voice of the language. */
  voiceId: string | null;
  onProgress?: (progress: SpeakProgress) => void;
}

export interface EngineVoice {
  id: string;
  name: string;
  lang: string;
}

export interface ReadAloudEngine {
  readonly id: EngineId;
  /** The voices this engine would use for a locale. Empty when there is none. */
  voices: (locale: string) => Promise<EngineVoice[]>;
  /**
   * Speaks the segments in order. Resolves when the last one has been spoken, or as soon as `stop`
   * is called (a stop is not a failure). Rejects with a `SpeechOutputError`.
   */
  speak: (segments: SpeakSegment[], options: SpeakOptions) => Promise<void>;
  /** Ends the speech in progress at once. Does nothing when nothing is spoken. */
  stop: () => void;
}

/** A failure of the voice, as a machine code the catalogue has a sentence for (`errors.<code>`):
 * `tts_no_local_voice` and `tts_failed` from an engine of the window, and whatever code Rust answered
 * for a voice worker (`speech_model_corrupt`, `speech_busy`...), which keeps its own sentence. */
export class SpeechOutputError extends Error {
  constructor(readonly code: string) {
    super(code);
    this.name = "SpeechOutputError";
  }
}

/** The machine code of whatever a speech or a preparation threw: a code from Rust or from an engine
 * is kept, anything else is the general failure (an English exception message never reaches the
 * screen). */
export function outputErrorCode(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    const { code } = error as { code: unknown };
    if (typeof code === "string" && code.length > 0) {
      return code;
    }
  }
  return "tts_failed";
}
