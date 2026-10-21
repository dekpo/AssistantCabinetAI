import { addScopeFiles, noDocumentsScope, tickedPaths, type Selection } from "./analysisScope";
import {
  entryIndexKey,
  familyOf,
  isTickable,
  type ExplorerEntry,
  type ExplorerFilter,
  type ExplorerRoot,
} from "./explorerModel";
import { indexKey } from "./indexKey";
import type { AnalysisScope, FileRecord, FinderDomain } from "./ipc";

/*
 * What the bottom bar of the explorer says and does about the selection, with no React: the number
 * of files used on each side, the files "Cocher les N" would add, and the short sentences for the
 * two ends of the choice. The selection itself is still the two scopes of `analysisScope.ts`, joined
 * only when a question is sent (`docs/SELECTION-AND-MEMORY.md`). Since explorer step 8 each scope is a
 * **family**'s: the documents scope holds the documents wherever they sit, the tables scope the
 * workbooks wherever they sit, each entry under the key the index knows the file by (`indexKey.ts`).
 * The chat contract is unchanged.
 */

/** The scope of each family, as the application holds them: `documents` is the documents' scope,
 * `data` the tables' scope. */
export type ScopesByFamily = Record<FinderDomain, AnalysisScope>;

/** The family whose scope holds a file: how it is read, not the folder it sits in. */
export function scopeFamilyOf(entry: ExplorerEntry): FinderDomain {
  return entry.family === "documents" ? "documents" : "data";
}

/** The record a scope is worked out over: the file's own, under the key the index knows it by. */
export function scopeRecord(entry: ExplorerEntry): FileRecord {
  return { ...entry.record, relativePath: entryIndexKey(entry) };
}

/** The analysed files of one family that can be ticked, in their records: what a scope is worked out
 * over. A file of the family counts wherever it sits. */
export function tickableRecords(
  entries: readonly ExplorerEntry[],
  family: FinderDomain,
): FileRecord[] {
  return entries
    .filter((entry) => scopeFamilyOf(entry) === family && isTickable(entry))
    .map(scopeRecord);
}

/** The analysed files of each family, read from the two folders' inventories, each under the key the index
 * knows it by (`indexKey.ts`): what a scope is worked out over when the explorer's own entries are not at
 * hand (the names picker adds files by the keys the knowledge base holds). */
export function analysedByFamily(
  documents: readonly FileRecord[],
  data: readonly FileRecord[],
): Record<FinderDomain, FileRecord[]> {
  const found: Record<FinderDomain, FileRecord[]> = { documents: [], data: [] };
  const folders: readonly (readonly [ExplorerRoot, readonly FileRecord[]])[] = [
    ["documents", documents],
    ["data", data],
  ];
  for (const [root, records] of folders) {
    for (const record of records) {
      const family = familyOf(record.extension);
      if (family === null || record.processingStatus !== "indexed") {
        continue;
      }
      const scopeFamily: FinderDomain = family === "documents" ? "documents" : "data";
      found[scopeFamily].push({
        ...record,
        relativePath: indexKey(root, scopeFamily, record.relativePath),
      });
    }
  }
  return found;
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
  scopes: ScopesByFamily,
): ExplorerEntry[] {
  const ticked = tickedKeysOf(scopes, all);
  return shown.filter(
    (entry) =>
      isTickable(entry) && !ticked[scopeFamilyOf(entry)].has(scopeRecord(entry).relativePath),
  );
}

/** The index keys each family's scope ticks, over the analysed files of the box. */
export function tickedKeysOf(
  scopes: ScopesByFamily,
  all: readonly ExplorerEntry[],
): Record<FinderDomain, Set<string>> {
  return {
    documents: new Set(tickedPaths(scopes.documents, tickableRecords(all, "documents"))),
    data: new Set(tickedPaths(scopes.data, tickableRecords(all, "data"))),
  };
}

/** Both scopes after `entries` were added: each family's own ticking rule, one family at a time. */
export function addEntriesToScopes(
  scopes: ScopesByFamily,
  entries: readonly ExplorerEntry[],
  all: readonly ExplorerEntry[],
  now: number,
): ScopesByFamily {
  const next = (family: FinderDomain): AnalysisScope =>
    addScopeFiles(
      scopes[family],
      entries
        .filter((entry) => scopeFamilyOf(entry) === family && isTickable(entry))
        .map(scopeRecord),
      tickableRecords(all, family),
      now,
    );
  return { documents: next("documents"), data: next("data") };
}

/** "Décocher": nothing is used on either side. */
export function clearedScopes(now: number): ScopesByFamily {
  return { documents: noDocumentsScope(now), data: noDocumentsScope(now) };
}

/** Whether the view is narrowed by anything: the typed search, the family, a format, a state, an
 * origin folder, a person, an organisation, a business type, a year or the conversation's files.
 * "Cocher les N" shows only then, as it ticks what the user is looking at. */
export function isFiltering(filter: ExplorerFilter): boolean {
  return (
    filter.found != null ||
    filter.family != null ||
    (filter.formats?.size ?? 0) > 0 ||
    (filter.states?.size ?? 0) > 0 ||
    (filter.folders?.size ?? 0) > 0 ||
    (filter.people?.size ?? 0) > 0 ||
    (filter.organisations?.size ?? 0) > 0 ||
    (filter.types?.size ?? 0) > 0 ||
    (filter.years?.size ?? 0) > 0 ||
    filter.ticked != null
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

/**
 * Whether a click on a file's row ticks it: anywhere on the row (the name, the badge, the empty part),
 * as the old lists did, except on a control of the row itself (the checkbox ticks itself, the Ouvrir
 * and Voir buttons do their own thing) and while the row cannot be ticked (not analysed, not its
 * folder, or a question being answered).
 */
export function rowClickTicks(click: { onControl: boolean; tickable: boolean }): boolean {
  return click.tickable && !click.onControl;
}
