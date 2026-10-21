import type { FinderReason, FinderResult } from "./ipc";

/** The pause in typing after which the search runs: the finder is cheap, but a result per
 * keystroke would make the list blink under her fingers. */
export const SEARCH_DELAY_MS = 250;

/** The most files a search returns for the panel: more than any folder holds. */
export const SEARCH_LIMIT = 100_000;

/** The files a result found, in its order. */
export function foundPaths(result: FinderResult): string[] {
  return result.files.map((hit) => hit.relativePath);
}

/** Why each file of a result was found, by path. */
export function foundReasons(result: FinderResult): Record<string, FinderReason[]> {
  return Object.fromEntries(result.files.map((hit) => [hit.relativePath, hit.reasons]));
}
