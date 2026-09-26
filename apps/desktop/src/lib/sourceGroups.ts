import type { Evidence } from "./ipc";

/** Every excerpt an answer drew from one file, folded into a single line for the reader. */
export interface SourceGroup {
  relativePath: string;
  /** Ascending, without repeats: two excerpts from one page are one page. */
  pages: number[];
  /** Whether any excerpt from this file was read by recognition rather than from its text layer. */
  ocr: boolean;
}

/**
 * One group per file, in the order each file first appears among the excerpts. The evidence itself
 * is untouched: this is only how it is listed, so "three excerpts from two files" reads as two
 * files instead of looking like three documents.
 */
export function groupSources(sources: Evidence[]): SourceGroup[] {
  const groups = new Map<string, SourceGroup>();
  for (const source of sources) {
    const group = groups.get(source.relativePath) ?? {
      relativePath: source.relativePath,
      pages: [],
      ocr: false,
    };
    if (!group.pages.includes(source.pageNumber)) {
      group.pages.push(source.pageNumber);
      group.pages.sort((a, b) => a - b);
    }
    group.ocr = group.ocr || source.origin === "ocr";
    groups.set(source.relativePath, group);
  }
  return [...groups.values()];
}
