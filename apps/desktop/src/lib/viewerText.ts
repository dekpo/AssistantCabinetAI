/**
 * What the plain text view shows. A text file can be as large as the 128 MiB the viewer reads, and
 * a page that lays out that many characters freezes the window, so the view shows a bounded part
 * and says so. The limit is a count of characters; it is the same for every text file.
 */
export const TEXT_VIEW_LIMIT = 500_000;

/** The longest a character can be in UTF-8; bounds how many bytes are decoded at all. */
const MAX_BYTES_PER_CHARACTER = 4;

export interface ViewerText {
  /** What is drawn: line breaks normalised, cut at a line end when the file is longer than the limit. */
  text: string;
  truncated: boolean;
  /** How many characters are drawn: what the notice of a cut file says. */
  shownCharacters: number;
}

/** CRLF and a lone CR become LF: a stored line break is one line break on screen. */
function normaliseBreaks(text: string): string {
  return text.replace(/\r\n?/g, "\n");
}

/**
 * Decodes UTF-8 bytes (Rust already turned Windows-1252 and UTF-16 into UTF-8 and dropped a byte-order
 * mark). Only the first part of a large file is decoded.
 */
export function prepareViewerText(
  bytes: ArrayBuffer | Uint8Array,
  limit: number = TEXT_VIEW_LIMIT,
): ViewerText {
  const all = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const window = Math.min(all.length, limit * MAX_BYTES_PER_CHARACTER + MAX_BYTES_PER_CHARACTER);
  const partial = window < all.length;
  /* `stream` keeps a character cut by the window from becoming a replacement character. */
  const decoded = normaliseBreaks(
    new TextDecoder("utf-8").decode(partial ? all.subarray(0, window) : all, { stream: partial }),
  );
  if (decoded.length <= limit && !partial) {
    return { text: decoded, truncated: false, shownCharacters: decoded.length };
  }
  const cut = decoded.slice(0, limit);
  const lastBreak = cut.lastIndexOf("\n");
  /* Back to the end of a line, unless the text is one huge line. */
  const text = lastBreak > limit / 2 ? cut.slice(0, lastBreak) : cut;
  return { text, truncated: true, shownCharacters: text.length };
}
