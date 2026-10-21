import { homeRoot, indexKey, parseIndexKey, type IndexFamily } from "./indexKey";
import type { FileRecord, FinderDomain } from "./ipc";

/*
 * What the file explorer knows and decides, with no React, no timer and no call, so that every rule
 * below is tested without a window (`explorerModel.test.ts`). The box merges the two folder
 * inventories into one flat list of entries, sorted into two families by extension wherever the file
 * sits on the disk (`docs/DECISIONS.md`, "the left panel becomes one file explorer").
 *
 * Nothing here reads the disk or the index, and nothing here knows the selection: an entry only
 * carries what the inventories already said about the file.
 */

/** The two physical folders an entry can come from. Same words as the finder's domains. */
export type ExplorerRoot = FinderDomain;

/** The two families a file is sorted into, by extension. */
export type Family = "documents" | "tables";

/** The three states the screen shows. The shape carries the state, not only the colour. */
export type ExplorerState = "analysed" | "toAnalyse" | "unreadable";

/** The extensions the document pipeline reads; mirrors `SUPPORTED_EXTENSIONS` in `discovery.rs`. */
export const DOCUMENT_EXTENSIONS: readonly string[] = [
  "pdf",
  "docx",
  "txt",
  "md",
  "jpg",
  "jpeg",
  "png",
  "vtt",
  "srt",
  "rtf",
  "eml",
  "odt",
  "odp",
  "pptx",
];

/** The extensions of the tabular pipeline; mirrors `FileKind::from_extension` in `file_record.rs`. */
export const TABLE_EXTENSIONS: readonly string[] = ["csv", "xlsx", "xls", "xlsm", "ods"];

/** The folder each family lives in: its own. A file of the other family can sit in the other folder; it
 * is analysed there, in place, by its own family's pass, and tagged with the folder it sits in. */
export const HOME_ROOT: Readonly<Record<Family, ExplorerRoot>> = {
  documents: "documents",
  tables: "data",
};

/** The family of an extension (lowercase, with or without the dot), or null when no pipeline of the
 * product reads it: such a file is kept apart, never listed under either family. */
export function familyOf(extension: string): Family | null {
  const key = extension.trim().replace(/^\./, "").toLowerCase();
  if (DOCUMENT_EXTENSIONS.includes(key)) {
    return "documents";
  }
  if (TABLE_EXTENSIONS.includes(key)) {
    return "tables";
  }
  return null;
}

/** One person or organisation a file is linked to, with every role that links them. */
export interface EntryLink {
  entityId: number;
  /** Machine codes: `author`, `subject`, `recipient`, `mention`... */
  roles: readonly string[];
}

/**
 * What the knowledge base and the profile know of an analysed file, as the filters read it. Attached
 * to the entries once it has been read (`attachFacts`); an entry without it (not analysed, or not read
 * yet) passes no filter of the knowledge base.
 */
export interface EntryFacts {
  /** The business type code, or null when the profile gave none. */
  type: string | null;
  /** `YYYY`, ascending. */
  years: readonly string[];
  links: readonly EntryLink[];
}

/** One file as the explorer lists it. */
export interface ExplorerEntry {
  /** `explorerKey(root, path)`: never the path alone, two folders can hold the same relative path. */
  key: string;
  root: ExplorerRoot;
  /** Relative to its root, as the inventory reports it. */
  path: string;
  name: string;
  stem: string;
  /** Lowercase, without the dot. */
  extension: string;
  family: Family;
  state: ExplorerState;
  /** The file sits in the folder of its own family. A file of the other family is shown under its
   * family, tagged with the folder it sits in, and analysed there by its own family's pass. */
  native: boolean;
  size: number;
  modified: number | null;
  /** The parent folder inside the root; empty for a file at the top. */
  folder: string;
  /** The inventory's own record: what opening, revealing and ticking the file need. */
  record: FileRecord;
  /** What the knowledge base knows of the file; absent until it has been read. */
  facts?: EntryFacts;
}

/** The key of a file: the root and the path, so the same relative path in both folders stays two. */
export function explorerKey(root: ExplorerRoot, path: string): string {
  return `${root}:${path}`;
}

function parentOf(path: string): string {
  const normal = path.replace(/\\/g, "/");
  const cut = normal.lastIndexOf("/");
  return cut < 0 ? "" : normal.slice(0, cut);
}

/**
 * The state of a file as the screen shows it, from what its inventory says: each folder's inventory
 * joins a file to what its own family made of it, wherever it sits. A file that changed since the last
 * analysis is "to analyse", so is one being analysed.
 */
export function stateOf(record: FileRecord): ExplorerState {
  switch (record.processingStatus) {
    case "indexed":
      return "analysed";
    case "failed":
      return "unreadable";
    default:
      return "toAnalyse";
  }
}

/** The entry of a record, or null when its extension belongs to neither family. */
export function entryOf(root: ExplorerRoot, record: FileRecord): ExplorerEntry | null {
  const family = familyOf(record.extension);
  if (family === null) {
    return null;
  }
  const native = HOME_ROOT[family] === root;
  return {
    key: explorerKey(root, record.relativePath),
    root,
    path: record.relativePath,
    name: record.name,
    stem: record.stem,
    extension: record.extension.toLowerCase(),
    family,
    state: stateOf(record),
    native,
    size: record.sizeBytes,
    modified: record.modifiedAt,
    folder: parentOf(record.relativePath),
    record,
  };
}

/** The merged view of the two folders. */
export interface MergedInventories {
  /** Every listed file, each under its family. Unsorted. */
  entries: ExplorerEntry[];
  /** Files no pipeline reads, kept apart: said once, never listed. */
  unsupported: number;
}

/** The folders on the disk, when both are chosen: only used to tell that two roots overlap. */
export interface RootPaths {
  documents: string | null;
  data: string | null;
}

function canonical(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

/**
 * The absolute identity of an entry, or null when the root is not known. Two roots that overlap (the
 * Data folder inside the Documents folder, or the same folder chosen twice) list the same physical
 * file from both; this is what tells them apart from two files that merely share a name.
 */
function physicalIdentity(
  root: ExplorerRoot,
  path: string,
  roots: RootPaths | undefined,
): string | null {
  const base = roots?.[root];
  if (base === null || base === undefined || base.trim() === "") {
    return null;
  }
  return `${canonical(base)}/${canonical(path)}`;
}

/**
 * The two inventories merged. A file listed from both roots (overlapping folders) is kept once: the
 * copy that sits in the folder of its own family wins, otherwise the one from the Documents folder.
 * Without the root paths nothing is de-duplicated: two files are two files.
 */
export function mergeInventories(
  documents: readonly FileRecord[],
  data: readonly FileRecord[],
  roots?: RootPaths,
): MergedInventories {
  const entries: ExplorerEntry[] = [];
  let unsupported = 0;
  const seen = new Map<string, number>();
  const seenUnsupported = new Set<string>();
  const add = (root: ExplorerRoot, records: readonly FileRecord[]) => {
    for (const record of records) {
      const identity = physicalIdentity(root, record.relativePath, roots);
      const entry = entryOf(root, record);
      if (entry === null) {
        // A file no pipeline reads, listed from both folders, is still one file.
        if (identity === null || !seenUnsupported.has(identity)) {
          unsupported += 1;
        }
        if (identity !== null) {
          seenUnsupported.add(identity);
        }
        continue;
      }
      const earlier = identity === null ? undefined : seen.get(identity);
      if (earlier === undefined) {
        if (identity !== null) {
          seen.set(identity, entries.length);
        }
        entries.push(entry);
      } else if (entry.native && entries[earlier]?.native === false) {
        entries[earlier] = entry;
      }
    }
  };
  add("documents", documents);
  add("data", data);
  return { entries, unsupported };
}

/** What narrows the view. Every field is optional: an empty filter keeps everything. */
export interface ExplorerFilter {
  family?: Family | null;
  /** Extensions, lowercase without the dot. Empty keeps every format. */
  formats?: ReadonlySet<string>;
  states?: ReadonlySet<ExplorerState>;
  /** Origin folders, as `folderKey(root, folder)`: a file passes when it sits in one of them or under
   * one of them. Empty keeps every folder. */
  folders?: ReadonlySet<string>;
  /** People, as `entityId` (any role) or `entityId:role`: a file passes when it is linked to one of them
   * in that way. Empty keeps every file. */
  people?: ReadonlySet<string>;
  /** Organisations, written like `people`. */
  organisations?: ReadonlySet<string>;
  /** Business type codes. */
  types?: ReadonlySet<string>;
  /** Years, `YYYY`: a file passes when one of its years is among them. */
  years?: ReadonlySet<string>;
  /** The keys the typed search found, or null while no search is active. */
  found?: ReadonlySet<string> | null;
  /** The keys of the files the conversation uses now, while the filter « Dans la conversation » is on;
   * null otherwise. */
  ticked?: ReadonlySet<string> | null;
}

/** An entity choice: `12` for any role, `12:author` for one role. */
export function entityChoice(entityId: number, role: string | null = null): string {
  return role === null ? String(entityId) : `${entityId}:${role}`;
}

/** The entity and the role (null for any role) of a choice made by `entityChoice`. */
export function parseEntityChoice(value: string): { entityId: number; role: string | null } {
  const cut = value.indexOf(":");
  const id = Number(cut < 0 ? value : value.slice(0, cut));
  return { entityId: Number.isFinite(id) ? id : -1, role: cut < 0 ? null : value.slice(cut + 1) };
}

/** Whether a file is linked to one of the chosen people or organisations (an empty choice keeps all). */
function linkedTo(entry: ExplorerEntry, chosen: ReadonlySet<string> | undefined): boolean {
  if (chosen === undefined || chosen.size === 0) {
    return true;
  }
  const links = entry.facts?.links ?? [];
  return [...chosen].some((value) => {
    const { entityId, role } = parseEntityChoice(value);
    return links.some(
      (link) => link.entityId === entityId && (role === null || link.roles.includes(role)),
    );
  });
}

/** The identity of an origin folder: the root and the folder inside it, because the two roots can
 * hold a sub-folder of the same name. */
export function folderKey(root: ExplorerRoot, folder: string): string {
  return `${root}:${folder}`;
}

/** Whether a file of `root` that sits in `folder` is in the origin folder `key`, or under it. */
function inFolder(entry: ExplorerEntry, key: string): boolean {
  const own = folderKey(entry.root, entry.folder);
  return own === key || own.startsWith(`${key}/`);
}

/** The entries that pass the filter, in their order. */
export function filterEntries(
  entries: readonly ExplorerEntry[],
  filter: ExplorerFilter,
): ExplorerEntry[] {
  return entries.filter(
    (entry) =>
      (filter.family == null || entry.family === filter.family) &&
      (filter.formats === undefined ||
        filter.formats.size === 0 ||
        filter.formats.has(entry.extension)) &&
      (filter.states === undefined || filter.states.size === 0 || filter.states.has(entry.state)) &&
      (filter.folders === undefined ||
        filter.folders.size === 0 ||
        [...filter.folders].some((key) => inFolder(entry, key))) &&
      linkedTo(entry, filter.people) &&
      linkedTo(entry, filter.organisations) &&
      (filter.types === undefined ||
        filter.types.size === 0 ||
        (entry.facts?.type != null && filter.types.has(entry.facts.type))) &&
      (filter.years === undefined ||
        filter.years.size === 0 ||
        (entry.facts?.years ?? []).some((year) => filter.years?.has(year))) &&
      (filter.ticked == null || filter.ticked.has(entry.key)) &&
      (filter.found == null || filter.found.has(entry.key)),
  );
}

/** The key the index knows a file by: its path when it sits in the folder of its own family, the folder
 * and the path otherwise (`indexKey.ts`). What a scope, a pass summary and the finder call the file. */
export function entryIndexKey(entry: ExplorerEntry): string {
  return indexKey(entry.root, entry.family === "documents" ? "documents" : "data", entry.path);
}

/** The explorer key of a file the index knows under `key` as a file of `family`: the key says which
 * folder it sits in (`indexKey.ts`), so a workbook found in the Documents folder is placed there. */
export function explorerKeyOfIndexKey(family: IndexFamily, key: string): string {
  const parsed = parseIndexKey(family, key);
  return parsed === null
    ? explorerKey(homeRoot(family), key)
    : explorerKey(parsed.root, parsed.path);
}

/** The keys a search found in each family, joined by the folder each file sits in; null while no search
 * is active in either. The finder searches by family across both folders, so a hit is an index key. */
export function foundKeys(
  documents: readonly string[] | null,
  data: readonly string[] | null,
): Set<string> | null {
  if (documents === null && data === null) {
    return null;
  }
  const keys = new Set<string>();
  for (const key of documents ?? []) {
    keys.add(explorerKeyOfIndexKey("documents", key));
  }
  for (const key of data ?? []) {
    keys.add(explorerKeyOfIndexKey("data", key));
  }
  return keys;
}

export type ExplorerSort = "name" | "date" | "state" | "relevance";

/** Analysed last: what asks for something comes first. */
const STATE_ORDER: Readonly<Record<ExplorerState, number>> = {
  toAnalyse: 0,
  unreadable: 1,
  analysed: 2,
};

const collator = new Intl.Collator("en", { numeric: true, sensitivity: "base" });

function byName(a: ExplorerEntry, b: ExplorerEntry): number {
  return collator.compare(a.name, b.name) || a.key.localeCompare(b.key);
}

/** A sorted copy. By name: numbers read as numbers, accents and case ignored. By date: newest
 * first, files without a date last. By state: to analyse, unreadable, analysed; each by name. */
export function sortEntries(
  entries: readonly ExplorerEntry[],
  sort: ExplorerSort,
  /** For `relevance`: the position the finder gave each file (`explorerKey`), best first. */
  rank?: ReadonlyMap<string, number>,
): ExplorerEntry[] {
  const copy = [...entries];
  switch (sort) {
    case "relevance":
      /* The finder's own order (best match first); a file it did not rank goes last, by name. */
      return copy.sort(
        (a, b) =>
          (rank?.get(a.key) ?? Number.MAX_SAFE_INTEGER) -
            (rank?.get(b.key) ?? Number.MAX_SAFE_INTEGER) || byName(a, b),
      );
    case "name":
      return copy.sort(byName);
    case "date":
      return copy.sort((a, b) => {
        if (a.modified === b.modified) {
          return byName(a, b);
        }
        if (a.modified === null) {
          return 1;
        }
        if (b.modified === null) {
          return -1;
        }
        return b.modified - a.modified;
      });
    case "state":
      return copy.sort((a, b) => STATE_ORDER[a.state] - STATE_ORDER[b.state] || byName(a, b));
  }
}

/** What a family's header counts. */
export interface FamilyCounts {
  total: number;
  analysed: number;
  toAnalyse: number;
  unreadable: number;
  /** What the family's Analyser button will do: every file of the family still to analyse, wherever it
   * sits (a file of the other folder is read in place by its own family's pass). */
  analysable: number;
  /** Files of the other folder listed under this family, whatever their state. */
  foreign: number;
}

export function countsByFamily(entries: readonly ExplorerEntry[]): Record<Family, FamilyCounts> {
  const empty = (): FamilyCounts => ({
    total: 0,
    analysed: 0,
    toAnalyse: 0,
    unreadable: 0,
    analysable: 0,
    foreign: 0,
  });
  const counts: Record<Family, FamilyCounts> = { documents: empty(), tables: empty() };
  for (const entry of entries) {
    const family = counts[entry.family];
    family.total += 1;
    family[entry.state] += 1;
    if (!entry.native) {
      family.foreign += 1;
    }
    if (entry.state === "toAnalyse") {
      family.analysable += 1;
    }
  }
  return counts;
}

/** Whether the file can be ticked: analysed, wherever it sits. */
export function isTickable(entry: ExplorerEntry): boolean {
  return entry.state === "analysed";
}

/** Whether the file can be opened in the centre: analysed, wherever it sits. The viewer reads it from
 * its physical root (`entry.root`, `entry.path`), which the viewer commands already accept for any
 * extension. */
export function isOpenable(entry: ExplorerEntry): boolean {
  return entry.state === "analysed";
}
/** Names that two or more listed files share (case-insensitive), so a grey hint can say where each
 * sits. Counted over every entry, whatever the family or the folder. */
export function duplicateNames(entries: readonly ExplorerEntry[]): Set<string> {
  const seen = new Map<string, number>();
  for (const entry of entries) {
    const name = entry.name.toLowerCase();
    seen.set(name, (seen.get(name) ?? 0) + 1);
  }
  return new Set([...seen].filter(([, count]) => count > 1).map(([name]) => name));
}

/** The last folder of the entry's path, shown in grey beside a name that is not unique; empty for a
 * file at the top of its root. */
export function folderHint(entry: ExplorerEntry): string {
  const cut = entry.folder.lastIndexOf("/");
  return cut < 0 ? entry.folder : entry.folder.slice(cut + 1);
}

/**
 * The order the lists take. While a text search is active, the finder's ranking (the best match
 * first, not the alphabet: the file looked for must not be the last of the list). With no search and
 * the Notes folder as the only folder choice, the newest first (a note is looked for by when it was
 * written). Otherwise by name. The « Récents » order, when asked for, is the newest first whatever the
 * rest.
 */
export function listSort(input: {
  searching: boolean;
  /** The folder choices (`folderKey`) and the key of the Notes folder, or null when there is none. */
  folders: ReadonlySet<string>;
  notesKey: string | null;
  /** The « Récents » order was asked for (a pill says so): it wins over the others, since the user
   * chose it and can remove it. */
  recent?: boolean;
}): ExplorerSort {
  if (input.recent === true) {
    return "date";
  }
  if (input.searching) {
    return "relevance";
  }
  const onlyNotes =
    input.notesKey !== null && input.folders.size === 1 && input.folders.has(input.notesKey);
  return onlyNotes ? "date" : "name";
}
