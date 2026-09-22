/**
 * Where an answer is in its life, and the one product rule that depends on it.
 *
 * A question may be stopped only while nothing has been written yet. Once the first words are on
 * screen the answer is hers to read, and the button is a "Send" again - disabled, because the
 * composer is empty. The decision lives here rather than in the button so a later change to the
 * view cannot quietly widen it (`docs/CHAT-UX-ASSESSMENT.md`, item 2).
 */

export type GenerationPhase = "idle" | "thinking" | "writing";

/** The machine code Rust answers with when she stopped the question herself. */
export const STOPPED_CODE = "chat_cancelled";

/** True only while the spinner is showing: that is the whole affordance. */
export function canStop(phase: GenerationPhase): boolean {
  return phase === "thinking";
}

/** The first piece of text has arrived, so the answer is being written and can no longer be
 * stopped. Any other phase is left alone: a delta cannot revive a finished run. */
export function phaseAfterFirstText(phase: GenerationPhase): GenerationPhase {
  return phase === "thinking" ? "writing" : phase;
}

/** A stop she asked for is not a failure: no banner, and the gateway is not re-checked. */
export function wasStopped(code: string): boolean {
  return code === STOPPED_CODE;
}
