/**
 * Copying the whole conversation to the clipboard. Pure and separate from `ChatPanel` so the
 * "is there anything to copy yet" rule and the text it produces are tested without a running
 * gateway, matching how `src/lib/turns.ts` holds the resend/regenerate rules.
 *
 * "Complete" excludes whichever entry is streaming right now: a partial answer being written is
 * not yet part of the conversation to hand someone else, even though she stopped it and it stays
 * on screen. It reads no differently once the run ends and `streamingId` goes back to null.
 */

interface Turn {
  id: string;
  role: "user" | "assistant";
  content: string;
}

/** Whether the button has anything to offer: one settled question and one settled answer. */
export function hasCopyableConversation(entries: Turn[], streamingId: string | null): boolean {
  const settled = entries.filter((entry) => entry.id !== streamingId && entry.content.trim().length > 0);
  return (
    settled.some((entry) => entry.role === "user") &&
    settled.some((entry) => entry.role === "assistant")
  );
}

/** The whole conversation as plain text, each turn labelled - the same content (markdown syntax
 * included) a single turn's own copy button puts on the clipboard.
 *
 * Compact on purpose (owner request, 5 October 2026): blank lines inside a turn are dropped, a
 * question and its answer follow each other on consecutive lines, and one blank line separates one
 * question-and-answer pair from the next. */
export function formatConversation(
  entries: Turn[],
  streamingId: string | null,
  authorLabel: (role: "user" | "assistant") => string,
): string {
  const settled = entries.filter((entry) => entry.id !== streamingId && entry.content.trim().length > 0);
  let text = "";
  settled.forEach((entry, index) => {
    const lines = `${authorLabel(entry.role)}: ${entry.content}`
      .split("\n")
      .map((line) => line.trimEnd())
      .filter((line) => line.trim().length > 0)
      .join("\n");
    const answersThePreviousQuestion = entry.role === "assistant" && settled[index - 1]?.role === "user";
    text += index === 0 ? lines : `${answersThePreviousQuestion ? "\n" : "\n\n"}${lines}`;
  });
  return text;
}
