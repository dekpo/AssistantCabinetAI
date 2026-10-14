import type { FileRecord, FinderResult } from "./ipc";

/** The pause in typing after which the search runs: the finder is cheap, but a result per
 * keystroke would change the selection under her fingers. */
export const SEARCH_DELAY_MS = 250;

/** The most files a search returns for the panel: more than any folder holds. */
export const SEARCH_LIMIT = 100_000;

/** The files a result found, in its order. */
export function foundPaths(result: FinderResult): string[] {
  return result.files.map((hit) => hit.relativePath);
}

/**
 * The listing under the field. While a search is active it shows the files found, and a control
 * ("tous les fichiers") shows the whole folder: a view only, the selection does not change.
 * Without a search it is the whole folder.
 */
export function shownFiles(
  files: FileRecord[],
  found: string[] | null,
  showAll: boolean,
): FileRecord[] {
  if (found === null || showAll) {
    return files;
  }
  const wanted = new Set(found);
  return files.filter((file) => wanted.has(file.relativePath));
}

/** How many of the files found are selected by the search: the analysed ones. */
export function selectedBySearch(found: string[], analysed: FileRecord[]): number {
  const wanted = new Set(found);
  return analysed.filter((file) => wanted.has(file.relativePath)).length;
}

/** How many of the files found cannot be selected because they are not analysed: found by their
 * name only, and said so. */
export function foundNotAnalysed(found: string[], analysed: FileRecord[]): number {
  return found.length - selectedBySearch(found, analysed);
}
