/**
 * Which prefix of the conversation an edited repost or a regenerate runs against
 * (`docs/CHAT-UX-ASSESSMENT.md` items 3-4). Pure and separate from `useChat` so the truncation
 * rule - not "which turn", the actual slicing - can be tested without a running gateway.
 *
 * Both discard everything from the affected turn onward and let `useChat` append a fresh
 * question and answer: no version history, truncate and replace, per the recorded decision.
 */

interface Turn {
  id: string;
  role: "user" | "assistant";
  content: string;
}

/** Entries before the edited question, or null when it is not found. */
export function truncateForResend<T extends Turn>(entries: T[], questionEntryId: string): T[] | null {
  const index = entries.findIndex((entry) => entry.id === questionEntryId);
  return index === -1 ? null : entries.slice(0, index);
}

/** Entries before the question that produced this answer, plus that question's text - or null
 * when the answer is not found or is not preceded by a user question (should not happen, but a
 * stray id must not silently resend the wrong thing). */
export function truncateForRegenerate<T extends Turn>(
  entries: T[],
  answerEntryId: string,
): { base: T[]; question: string } | null {
  const index = entries.findIndex((entry) => entry.id === answerEntryId);
  const question = index > 0 ? entries[index - 1] : undefined;
  if (question === undefined || question.role !== "user") {
    return null;
  }
  return { base: entries.slice(0, index - 1), question: question.content };
}
