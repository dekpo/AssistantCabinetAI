import type { ChatTurn } from "./ipc";

/**
 * What the model is given of the conversation so far (`docs/SELECTION-AND-MEMORY.md`).
 *
 * The text she saw, question and answer, and nothing else: no sources, no excerpts, no timings.
 * A turn the software wrote itself - "analyse your documents first" - is not a model answer and is
 * left out, together with the question it answered. Rust decides how much of what is sent fits the
 * chosen model, and drops anything that is not an answered question, so this only has to say what
 * the conversation was.
 */

interface Turn {
  role: "user" | "assistant";
  content: string;
  /** The software's own message rather than an answer. */
  needsIndexing?: true;
}

export function conversationHistory(entries: Turn[]): ChatTurn[] {
  const history: ChatTurn[] = [];
  for (const [index, entry] of entries.entries()) {
    const next = entries[index + 1];
    if (
      entry.role === "user" &&
      next !== undefined &&
      next.role === "assistant" &&
      next.needsIndexing !== true &&
      entry.content.trim().length > 0 &&
      next.content.trim().length > 0
    ) {
      history.push({ role: "user", content: entry.content }, { role: "assistant", content: next.content });
    }
  }
  return history;
}
