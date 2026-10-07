import type { AnalysisScope, CombinedScope, FileRecord } from "./ipc";

/**
 * The files a conversation may rely on, as she ticked them (`docs/SELECTION-AND-MEMORY.md`). One
 * set of rules for both cards: the Documents Folder's documents and the Data Folder's workbooks
 * are chosen the same way, each list relative to its own folder.
 *
 * Three states, and the wire form Rust already understands for each:
 * - **none**: an explicit selection with no entries. The default. Questions are answered without
 *   documents, and say so.
 * - **some**: an explicit selection of the files she ticked, each pinned to its content.
 * - **all**: the whole folder, including documents analysed later.
 */
export type Selection = "none" | "some" | "all";

/** "Documents utilisés : aucun". The default, at startup and whenever the folder changes. */
export function noDocumentsScope(now: number): AnalysisScope {
  return { mode: { kind: "explicit", entries: [] }, createdAt: now, updatedAt: now };
}

/** "Documents utilisés : tous". */
export function wholeFolderScope(now: number): AnalysisScope {
  return { mode: { kind: "whole_folder" }, createdAt: now, updatedAt: now };
}

export function selectionOf(scope: AnalysisScope): Selection {
  if (scope.mode.kind === "whole_folder") {
    return "all";
  }
  return scope.mode.entries.length === 0 ? "none" : "some";
}

/** The relative paths an explicit selection keeps; empty for none and for the whole folder. */
export function scopedPaths(scope: AnalysisScope): string[] {
  return scope.mode.kind === "explicit"
    ? scope.mode.entries.map((entry) => entry.relativePath)
    : [];
}

/** The files the listing shows ticked. "Tous" ticks every analysed file. */
export function tickedPaths(scope: AnalysisScope, analysed: FileRecord[]): string[] {
  return scope.mode.kind === "whole_folder"
    ? analysed.map((file) => file.relativePath)
    : scopedPaths(scope);
}

/**
 * The selection after `file` is ticked or unticked, among the `analysed` files the listing offers.
 *
 * Ticking pins the file's `id` at that moment, so Rust can tell when it was replaced afterwards.
 * Unticking one file out of "tous" keeps every other analysed file, each pinned now. Ticking the
 * last analysed file is "tous": to her it means the same thing. Unticking the last file is
 * "aucun", never the whole folder - a selection only ever widens because she widened it.
 */
export function toggleScopeFile(
  scope: AnalysisScope,
  file: FileRecord,
  analysed: FileRecord[],
  now: number,
): AnalysisScope {
  const entries =
    scope.mode.kind === "explicit"
      ? scope.mode.entries
      : analysed.map((each) => ({ relativePath: each.relativePath, pinnedId: each.id, addedAt: now }));
  const present = entries.some((entry) => entry.relativePath === file.relativePath);
  const next = present
    ? entries.filter((entry) => entry.relativePath !== file.relativePath)
    : [...entries, { relativePath: file.relativePath, pinnedId: file.id, addedAt: now }];

  if (next.length === 0) {
    return { ...noDocumentsScope(scope.createdAt), updatedAt: now };
  }
  const everyAnalysedTicked =
    analysed.length > 0 &&
    analysed.every((each) => next.some((entry) => entry.relativePath === each.relativePath));
  if (everyAnalysedTicked) {
    return { ...wholeFolderScope(scope.createdAt), updatedAt: now };
  }
  return { mode: { kind: "explicit", entries: next }, createdAt: scope.createdAt, updatedAt: now };
}

/** The header checkbox: from "tous" to "aucun", and from anything else to "tous". */
export function toggleAll(scope: AnalysisScope, now: number): AnalysisScope {
  return selectionOf(scope) === "all"
    ? { ...noDocumentsScope(scope.createdAt), updatedAt: now }
    : { ...wholeFolderScope(scope.createdAt), updatedAt: now };
}

/**
 * What is sent with a question: both lists, built fresh before each one. The timestamps are the
 * later of the two, since the combined choice last changed when either list did.
 */
export function combineScopes(documents: AnalysisScope, data: AnalysisScope): CombinedScope {
  return {
    mode: documents.mode,
    dataMode: data.mode,
    createdAt: Math.min(documents.createdAt, data.createdAt),
    updatedAt: Math.max(documents.updatedAt, data.updatedAt),
  };
}
