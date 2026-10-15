import type { Evidence } from "./ipc";

/** Every excerpt an answer drew from one file, folded into a single line for the reader. */
export interface SourceGroup {
  relativePath: string;
  /** Ascending, without repeats: two excerpts from one page are one page. */
  pages: number[];
  /** Titles of the sections the excerpts start in, in order, without repeats; none for a document
   * with no structure. */
  sections: string[];
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
      sections: [],
      ocr: false,
    };
    // Every page the passage covers: one that runs over a page break is cited on both.
    for (let page = source.pageNumber; page <= (source.lastPage ?? source.pageNumber); page += 1) {
      if (!group.pages.includes(page)) {
        group.pages.push(page);
      }
    }
    group.pages.sort((a, b) => a - b);
    const title = source.section?.title;
    if (title !== undefined && title !== "" && !group.sections.includes(title)) {
      group.sections.push(title);
    }
    group.ocr = group.ocr || source.origin === "ocr";
    groups.set(source.relativePath, group);
  }
  return [...groups.values()];
}
