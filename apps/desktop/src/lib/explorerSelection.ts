import { addScopeFiles, noDocumentsScope, tickedPaths, type Selection } from "./analysisScope";
import {
  isTickable,
  type ExplorerEntry,
  type ExplorerFilter,
  type ExplorerRoot,
} from "./explorerModel";
import type { AnalysisScope, FileRecord } from "./ipc";

/*
 * What the bottom bar of the explorer says and does about the selection, with no React: the number
 * of files used on each side, the files "Cocher les N" would add, and the short sentences for the
 * two ends of the choice. The selection itself is still the two scopes of `analysisScope.ts`, one per
 * folder, joined only when a question is sent (`docs/SELECTION-AND-MEMORY.md`); nothing here changes
 * that contract.
 */

/** The scope of each folder, as the application holds them. */
export type ScopesByRoot = Record<ExplorerRoot, AnalysisScope>;

/** The analysed files of one folder that can be ticked, in their records: what a scope is worked out
 * over. A file of the other family is never among them. */
export function tickableRecords(
  entries: readonly ExplorerEntry[],
  root: ExplorerRoot,
): FileRecord[] {
  return entries.filter((entry) => entry.root === root && isTickable(entry)).map((e) => e.record);
}

/** How many files the selection uses in one folder: the ones ticked, or every analysed file for
 * "tous". */
export function usedCount(scope: AnalysisScope, tickable: readonly FileRecord[]): number {
  return tickedPaths(scope, [...tickable]).length;
}

/** The shown files that could be ticked and are not yet: what "Cocher les N" adds. A side already
 * on "tous" has none left to add. */
export function entriesToTick(
  shown: readonly ExplorerEntry[],
  all: readonly ExplorerEntry[],
  scopes: ScopesByRoot,
): ExplorerEntry[] {
  const ticked: Record<ExplorerRoot, Set<string>> = {
    documents: new Set(tickedPaths(scopes.documents, tickableRecords(all, "documents"))),
    data: new Set(tickedPaths(scopes.data, tickableRecords(all, "data"))),
  };
  return shown.filter((entry) => isTickable(entry) && !ticked[entry.root].has(entry.path));
}

/** Both scopes after `entries` were added: each folder's own ticking rule, one folder at a time. */
export function addEntriesToScopes(
  scopes: ScopesByRoot,
  entries: readonly ExplorerEntry[],
  all: readonly ExplorerEntry[],
  now: number,
): ScopesByRoot {
  const next = (root: ExplorerRoot): AnalysisScope =>
    addScopeFiles(
      scopes[root],
      entries.filter((entry) => entry.root === root && isTickable(entry)).map((e) => e.record),
      tickableRecords(all, root),
      now,
    );
  return { documents: next("documents"), data: next("data") };
}

/** "Décocher": nothing is used on either side. */
export function clearedScopes(now: number): ScopesByRoot {
  return { documents: noDocumentsScope(now), data: noDocumentsScope(now) };
}

/** Whether the view is narrowed by anything: the typed search, the family, a format or a state.
 * "Cocher les N" shows only then, as it ticks what the user is looking at. */
export function isFiltering(filter: ExplorerFilter): boolean {
  return (
    filter.found != null ||
    filter.family != null ||
    (filter.formats?.size ?? 0) > 0 ||
    (filter.states?.size ?? 0) > 0
  );
}

/** A second line of the bottom bar, only when there is something to say. */
export type SelectionNote = "none" | "wholeDocuments" | "wholeTables";

/** Nothing used on either side, or a whole family used: the two ends of the choice, each warned
 * about for its own reason. With nothing used the answers do not rest on the files at all; with a
 * whole family the model has more to sift than it can use well. */
export function selectionNotes(documents: Selection, data: Selection): SelectionNote[] {
  if (documents === "none" && data === "none") {
    return ["none"];
  }
  const notes: SelectionNote[] = [];
  if (documents === "all") {
    notes.push("wholeDocuments");
  }
  if (data === "all") {
    notes.push("wholeTables");
  }
  return notes;
}
