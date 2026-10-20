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

/** The folder each family is analysed from today: its own. A file of the other family sits in the
 * other folder and cannot be analysed yet (`EXPLORER-08`). */
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
  /** The file sits in the folder of its own family: it can be analysed and ticked. A file of the
   * other family is shown under its family, tagged, and cannot be analysed yet. */
  native: boolean;
  size: number;
  modified: number | null;
  /** The parent folder inside the root; empty for a file at the top. */
  folder: string;
  /** The inventory's own record: what opening, revealing and ticking the file need. */
  record: FileRecord;
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
 * The state of a file as the screen shows it. A file of the other family is always "to analyse":
 * what its own inventory says about it (a document forced to `failed` in the Data folder, a workbook
 * "not assessed" in the Documents folder) describes a pipeline that is not its own. A file that
 * changed since the last analysis is "to analyse" too, so is one being analysed.
 */
export function stateOf(record: FileRecord, native: boolean): ExplorerState {
  if (!native) {
    return "toAnalyse";
  }
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
    state: stateOf(record, native),
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
function physicalIdentity(entry: ExplorerEntry, roots: RootPaths | undefined): string | null {
  const base = roots?.[entry.root];
  if (base === null || base === undefined || base.trim() === "") {
    return null;
  }
  return `${canonical(base)}/${canonical(entry.path)}`;
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
  const add = (root: ExplorerRoot, records: readonly FileRecord[]) => {
    for (const record of records) {
      const entry = entryOf(root, record);
      if (entry === null) {
        unsupported += 1;
        continue;
      }
      const identity = physicalIdentity(entry, roots);
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
  /** The keys the typed search found, or null while no search is active. */
  found?: ReadonlySet<string> | null;
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
      (filter.found == null || filter.found.has(entry.key)),
  );
}

/** The keys a search found in each folder, joined; null while no search is active in either. */
export function foundKeys(
  documents: readonly string[] | null,
  data: readonly string[] | null,
): Set<string> | null {
  if (documents === null && data === null) {
    return null;
  }
  const keys = new Set<string>();
  for (const path of documents ?? []) {
    keys.add(explorerKey("documents", path));
  }
  for (const path of data ?? []) {
    keys.add(explorerKey("data", path));
  }
  return keys;
}

export type ExplorerSort = "name" | "date" | "state";

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
): ExplorerEntry[] {
  const copy = [...entries];
  switch (sort) {
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
  /** What the family's Analyser button will do: files of its own folder still to analyse. */
  analysable: number;
  /** Files of the other folder waiting under this family: shown, not counted in `analysable`. */
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
    } else if (entry.state === "toAnalyse") {
      family.analysable += 1;
    }
  }
  return counts;
}

/** Whether the file can be ticked: analysed, and in the folder of its own family. */
export function isTickable(entry: ExplorerEntry): boolean {
  return entry.native && entry.state === "analysed";
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
