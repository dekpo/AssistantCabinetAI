/**
 * Where the words of a dictation go in the question box. Pure functions of the draft, the place the
 * caret was when the recording started, and the transcript: the box is changed by the caller, who
 * keeps the result for the one-step Undo. A transcript only ever fills the box; nothing here sends.
 */

/** The box as it was when the recording started: its text and what was selected. */
export interface RememberedSelection {
  draft: string;
  start: number;
  end: number;
}

export interface Insertion {
  /** The whole draft after the insertion. */
  text: string;
  /** Where the caret goes: just after the words and the space that follows them, if one was added. */
  caret: number;
  /** The draft and caret as they were before, for the Undo. */
  before: { text: string; caret: number };
}

/** Clamps a selection to the draft, so a stale or reversed range cannot slice outside it. */
export function rememberSelection(draft: string, start: number, end: number): RememberedSelection {
  const from = Math.min(Math.max(0, Math.min(start, end)), draft.length);
  const to = Math.min(Math.max(0, Math.max(start, end)), draft.length);
  return { draft, start: from, end: to };
}

/** Characters after which a word needs no space before it. */
const OPENERS = new Set(["(", "[", "{", "«", "'", "’", '"', "“"]);
/** Characters before which a word needs no space after it. */
const CLOSERS = new Set([".", ",", ";", ":", "!", "?", ")", "]", "}", "»", "%", "…", "'", '"', "”"]);

function isSpace(character: string): boolean {
  return /\s/.test(character);
}

/**
 * Puts the transcript in the box. If the draft is the one the recording started from, the words
 * replace the remembered selection (or sit at the caret when nothing was selected); if the draft
 * changed while she spoke, they are appended at the end instead, so nothing she typed is overwritten.
 * A space is added on a side only when a word would otherwise touch the new words. An empty
 * transcript changes nothing and gives `null`.
 */
export function insertTranscript(
  draft: string,
  transcript: string,
  remembered: RememberedSelection | null,
): Insertion | null {
  const words = transcript.trim();
  if (words.length === 0) {
    return null;
  }
  const replacing = remembered !== null && remembered.draft === draft;
  const start = replacing ? remembered.start : draft.length;
  const end = replacing ? remembered.end : draft.length;
  const left = start > 0 ? (draft[start - 1] as string) : null;
  const right = end < draft.length ? (draft[end] as string) : null;
  const before = left !== null && !isSpace(left) && !OPENERS.has(left) ? " " : "";
  const after = right !== null && !isSpace(right) && !CLOSERS.has(right) ? " " : "";
  const text = draft.slice(0, start) + before + words + after + draft.slice(end);
  return {
    text,
    caret: start + before.length + words.length + after.length,
    before: {
      text: draft,
      caret: replacing ? remembered.start : draft.length,
    },
  };
}

/** The Undo is offered only while the box still holds exactly what the insertion made. */
export function canUndo(draft: string, insertion: Insertion): boolean {
  return draft === insertion.text;
}

/** How many numbers (runs of digits, with their decimal or thousands separators) a text holds. */
export function countNumbers(text: string): number {
  return (text.match(/\d+(?:[.,  ]\d+)*/g) ?? []).length;
}
