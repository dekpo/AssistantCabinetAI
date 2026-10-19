/**
 * Where the words of a dictated passage go in a note. Pure functions of the note's text and the
 * passage: the editor changes the note with the result. Nothing here reads, writes or sends.
 */

/** A passage that starts with one of these glues itself to the word before it. */
const CLOSERS = new Set([",", ".", ";", ":", "!", "?", "…", ")", "]", "»"]);
/** The punctuation a passage that starts with a mark replaces at the end of the text before it. */
const SENTENCE_MARKS = new Set([",", ".", ";", ":", "!", "?", "…"]);
/** After one of these the next word needs no blank. */
const OPENERS = new Set(["(", "[", "«", '"']);

export interface Appended {
  /** The whole note after the passage went in. */
  text: string;
  /** Where the words of the passage begin in `text`. */
  start: number;
  /** The words of the passage as they stand in `text` from `start`. */
  words: string;
}

function trailingNewlines(text: string): number {
  return text.length - text.replace(/\n+$/, "").length;
}

/**
 * Appends a passage at the end of a note. The passage may open with line breaks (a title, a new
 * line): they are joined to the note's own so that the result has exactly as many as the passage
 * asked for. Otherwise a blank goes between the two, except before a mark and after an opening one.
 * An empty passage changes nothing; the very start of a note drops its leading blanks.
 */
export function appendToNote(existing: string, piece: string): Appended {
  const body = piece.replace(/^\s+/, "");
  if (body === "" && !piece.includes("\n")) {
    return { text: existing, start: existing.length, words: "" };
  }
  if (existing.trim() === "") {
    return { text: body, start: 0, words: body.trimEnd() };
  }
  const leading = piece.slice(0, piece.length - body.length);
  const breaks = (leading.match(/\n/g) ?? []).length;
  if (breaks > 0) {
    const base = existing.replace(/[ \t]+$/, "");
    /* One break inside a paragraph is a hard break (two blanks), the only kind the preview draws. */
    const hard = breaks === 1 && trailingNewlines(base) === 0 ? "  " : "";
    const prefix =
      base + hard + "\n".repeat(Math.max(0, breaks - trailingNewlines(base)));
    return { text: prefix + body, start: prefix.length, words: body.trimEnd() };
  }
  const last = existing.charAt(existing.length - 1);
  const first = body.charAt(0);
  let base = existing;
  let joint = " ";
  if (/\s/.test(last) || OPENERS.has(last)) {
    joint = "";
  } else if (CLOSERS.has(first)) {
    joint = "";
    if (SENTENCE_MARKS.has(first)) {
      base = existing.replace(/[,.;:!?…]+$/, "");
    }
  }
  const prefix = base + joint;
  return { text: prefix + body, start: prefix.length, words: body.trimEnd() };
}
