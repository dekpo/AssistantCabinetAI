/**
 * Where an answer is in its life, and what stopping it does at each point.
 *
 * Two stops, because they are two different acts. Before a single word exists there is nothing to
 * keep, so the turn goes and the question comes back to the composer. Once words are on screen they
 * are hers: stopping then leaves them exactly as written, marked as interrupted. That second one is
 * not a convenience - nothing in the chain caps how long an answer may be, and a small model can
 * fall into a repetition loop and write the same block until something disconnects
 * (`docs/TROUBLESHOOTING.md`, 22 September 2026).
 *
 * The rule lives here rather than in a button so a later change to the view cannot quietly widen
 * it, and so what each stop *does* is decided once (`docs/CHAT-UX-ASSESSMENT.md`, item 2).
 */

export type GenerationPhase = "idle" | "thinking" | "writing";

/** What a stop asked for right now would leave behind. */
export type StopOutcome = "discardTurn" | "keepPartialAnswer";

/** The machine code Rust answers with when she stopped the question herself. */
export const STOPPED_CODE = "chat_cancelled";

/** Null when there is nothing to stop, which is also what hides both stop affordances. */
export function stopOutcome(phase: GenerationPhase): StopOutcome | null {
  switch (phase) {
    case "thinking":
      return "discardTurn";
    case "writing":
      return "keepPartialAnswer";
    case "idle":
      return null;
  }
}

/** The stop below the composer: it only ever abandons a question nothing has answered yet. */
export function canDiscard(phase: GenerationPhase): boolean {
  return stopOutcome(phase) === "discardTurn";
}

/** The stop under the answer itself: it ends the writing and keeps what is already there. */
export function canInterrupt(phase: GenerationPhase): boolean {
  return stopOutcome(phase) === "keepPartialAnswer";
}

/** The first piece of text has arrived, so the answer is being written. Any other phase is left
 * alone: a delta cannot revive a finished run. */
export function phaseAfterFirstText(phase: GenerationPhase): GenerationPhase {
  return phase === "thinking" ? "writing" : phase;
}

/** A stop she asked for is not a failure: no banner, and the gateway is not re-checked. */
export function wasStopped(code: string): boolean {
  return code === STOPPED_CODE;
}
