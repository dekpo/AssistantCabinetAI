import type { Limit } from "../lib/knowledgeSettings";

/**
 * The bounds Rust clamps the speech settings to (`speech/mod.rs`), repeated once for the controls:
 * a number input has to know them before anything is saved. `limits.test.ts` reads the Rust source
 * and fails if the two drift apart. Rust remains the one that decides; after a save the field shows
 * the value it sent back.
 */
export const SPEECH_LIMITS: {
  /** Cap of one chat dictation, in seconds. */
  dictationSeconds: Limit;
  /** Cap of one note dictation, in seconds. */
  noteSeconds: Limit;
  /** Read-aloud speed, as a multiple of the natural rate. */
  ttsRate: Limit;
} = {
  dictationSeconds: { min: 10, max: 600 },
  noteSeconds: { min: 60, max: 3600 },
  ttsRate: { min: 0.5, max: 2 },
};
