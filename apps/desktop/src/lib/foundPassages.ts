import type { Evidence, MatchSpan } from "./ipc";

/** A piece of an excerpt shown to the reader: `marked` is a word of the question. */
export interface SnippetPart {
  text: string;
  marked: boolean;
}

/** One passage the search found, as listed in « Passages trouvés ». */
export interface FoundPassage {
  chunkId: string;
  section: string | null;
  firstPage: number;
  lastPage: number;
  /** No word of the question is in the passage: it was found by its meaning. Never highlighted. */
  byMeaning: boolean;
  snippet: SnippetPart[];
}

export interface FoundDocument {
  relativePath: string;
  passages: FoundPassage[];
}

export interface FoundPassages {
  /** Files with at least one passage holding a word of the question, in the order they were found. */
  matching: FoundDocument[];
  /** Files found by meaning alone, folded under the others. */
  byMeaning: FoundDocument[];
  passageCount: number;
  documentCount: number;
}

const ELLIPSIS = "…";

/**
 * A short piece of `text` around the first words of the question, those words marked. Offsets are in
 * characters (code points), as the engine counts them, not in UTF-16 units. A passage with no word
 * of the question shows its beginning, nothing marked.
 */
export function snippetOf(text: string, matches: MatchSpan[], radius: number): SnippetPart[] {
  const chars = Array.from(text);
  const spans = [...matches]
    .filter((span) => span.start < span.end && span.end <= chars.length)
    .sort((a, b) => a.start - b.start);
  const head = spans[0];
  const from = Math.max(0, (head?.start ?? 0) - radius);
  // Without a word the beginning of the passage is shown, as long as a window around a word.
  const to = Math.min(chars.length, head !== undefined ? head.end + radius : radius * 2);
  const parts: SnippetPart[] = [];
  const push = (value: string, marked: boolean) => {
    if (value !== "") {
      parts.push({ text: value, marked });
    }
  };
  push(from > 0 ? ELLIPSIS : "", false);
  let cursor = from;
  for (const span of spans) {
    if (span.start >= to) {
      break;
    }
    if (span.start < cursor) {
      continue;
    }
    push(chars.slice(cursor, span.start).join(""), false);
    push(chars.slice(span.start, Math.min(span.end, to)).join(""), true);
    cursor = Math.min(span.end, to);
  }
  push(chars.slice(cursor, to).join(""), false);
  push(to < chars.length ? ELLIPSIS : "", false);
  return parts;
}

/**
 * The passages of an answer for the reader, built from the retrieval alone (so it is complete before
 * the model writes a word): one entry per file, the files that hold a word of the question first, the
 * files found by meaning alone apart.
 */
export function foundPassages(sources: Evidence[]): FoundPassages {
  const files = new Map<string, FoundDocument>();
  const withWords = new Set<string>();
  for (const source of sources) {
    const byMeaning = source.matches.length === 0;
    if (!byMeaning) {
      withWords.add(source.relativePath);
    }
    const file = files.get(source.relativePath) ?? { relativePath: source.relativePath, passages: [] };
    const title = source.section?.title;
    file.passages.push({
      chunkId: source.chunkId,
      section: title !== undefined && title !== "" ? title : null,
      firstPage: source.pageNumber,
      lastPage: Math.max(source.pageNumber, source.lastPage ?? source.pageNumber),
      byMeaning,
      snippet: snippetOf(source.text, source.matches, 60),
    });
    files.set(source.relativePath, file);
  }
  const all = [...files.values()];
  return {
    matching: all.filter((file) => withWords.has(file.relativePath)),
    byMeaning: all.filter((file) => !withWords.has(file.relativePath)),
    passageCount: sources.length,
    documentCount: all.length,
  };
}
