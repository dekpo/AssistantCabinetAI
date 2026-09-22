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

/** The whole conversation as plain text, each turn labelled and separated by a blank line - the
 * same raw content (markdown syntax included) a single turn's own copy button already puts on the
 * clipboard, so the two behave the same way once pasted somewhere else. */
export function formatConversation(
  entries: Turn[],
  streamingId: string | null,
  authorLabel: (role: "user" | "assistant") => string,
): string {
  return entries
    .filter((entry) => entry.id !== streamingId && entry.content.trim().length > 0)
    .map((entry) => `${authorLabel(entry.role)}: ${entry.content}`)
    .join("\n\n");
}
