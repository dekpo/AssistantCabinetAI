import type { AppError } from "./errors";
import {
  countsByFamily,
  entryIndexKey,
  explorerKeyOfIndexKey,
  isOpenable,
  sortEntries,
  type ExplorerEntry,
  type Family,
  type FamilyCounts,
} from "./explorerModel";
import type { FolderState } from "./explorerStates";
import { viewShowingState, type ExplorerView } from "./explorerView";
import type { IndexSummary, KnowledgeSummary, RenamedFile, UnreadFormat } from "./ipc";
import { CAPABILITY_KEYS } from "./passSummary";

/*
 * The « Aperçu » page of the centre, as data: what the two folders hold and what the last pass of each
 * family reported go in, an ordered list of blocks comes out. No React, no timer, no translator and no
 * call, so every state of the page is tested without a window (`overviewModel.test.ts`). The page
 * component only draws the blocks, in her language, through the catalogues.
 */

/** The families in the order the page lists them. */
const FAMILIES: readonly Family[] = ["documents", "tables"];

/** Files to check listed one by one; the rest is counted, and the link shows them all on the left. */
export const MAX_CHECK_LISTED = 20;

/** What one family's pass left, and what its folder holds. */
export interface FamilyReportInput {
  /** The folder of the family, or null while none is chosen. */
  folder: string | null;
  folderState: FolderState;
  /** What the last finished pass of this family reported; null before one, after a reset, and while
   * a new pass runs. */
  summary: IndexSummary | null;
  /** How long that pass took, from the press to its end, as the client measured it. */
  durationMs: number | null;
  /** When it ended (epoch milliseconds). */
  finishedAt: number | null;
  running: boolean;
  /** The pass failed before it produced a summary. */
  error: AppError | null;
  /** The share of the meaning search computed, while it is incomplete. */
  meaningPercent: number | null;
  /** The index predates the file profile: only an Analyse fills it. */
  needsAnalyse: boolean;
}

export interface OverviewInput {
  /** Every listed file of both folders, as the explorer merges them. */
  entries: readonly ExplorerEntry[];
  documents: FamilyReportInput;
  tables: FamilyReportInput;
}

/**
 * What the explorer knows of the two folders that the page needs and `App` cannot read by itself:
 * the merged files, the situation of each folder, and the two cheap lookups of the index (the share
 * of the meaning search computed, whether the file profile is missing).
 */
export interface ExplorerFacts {
  entries: readonly ExplorerEntry[];
  documentsState: FolderState;
  tablesState: FolderState;
  meaningPercent: number | null;
  documentsNeedsAnalyse: boolean;
  tablesNeedsAnalyse: boolean;
}

/** What one family's pass hook holds. */
export interface PassFacts {
  summary: IndexSummary | null;
  durationMs: number | null;
  finishedAt: number | null;
  running: boolean;
  error: AppError | null;
}

/**
 * The page's input from the explorer's facts and the two passes. Before the explorer has reported,
 * a chosen folder counts as holding files (as the box does while its inventory is read) and no file
 * is known.
 */
export function overviewInput(
  facts: ExplorerFacts | null,
  folders: { documents: string | null; tables: string | null },
  documents: PassFacts,
  tables: PassFacts,
): OverviewInput {
  const assumed = (folder: string | null): FolderState => (folder === null ? "none" : "filled");
  return {
    entries: facts?.entries ?? [],
    documents: {
      ...documents,
      folder: folders.documents,
      folderState: facts?.documentsState ?? assumed(folders.documents),
      meaningPercent: facts?.meaningPercent ?? null,
      needsAnalyse: facts?.documentsNeedsAnalyse ?? false,
    },
    tables: {
      ...tables,
      folder: folders.tables,
      folderState: facts?.tablesState ?? assumed(folders.tables),
      meaningPercent: null,
      needsAnalyse: facts?.tablesNeedsAnalyse ?? false,
    },
  };
}

export type PassOutcome = "done" | "stopped" | "nothing";

export interface LastPass {
  at: number | null;
  durationMs: number | null;
  outcome: PassOutcome;
}

export interface FamilyFigures {
  family: Family;
  /** The whole path of the family's folder, as the settings hold it. */
  folder: string | null;
  folderState: FolderState;
  running: boolean;
  analysed: number;
  unreadable: number;
  toAnalyse: number;
  meaningPercent: number | null;
  needsAnalyse: boolean;
  /** The last pass of this session, or null when none ran since the window opened. */
  lastPass: LastPass | null;
}

/** Why a file is in the list to check. The sentence is the page's, from the catalogue. */
export type CheckReason =
  | { kind: "error"; code: string; data: Record<string, unknown> }
  /** The file was read and holds no text, and no text recognition ran on it. */
  | { kind: "empty" }
  /** Text recognition ran on the file and found no readable text (a blank or unreadable scan). */
  | { kind: "recognised" }
  | { kind: "lowConfidence" }
  | { kind: "noTable" };

/** What can be done with a file of the list: the same gates as the explorer's own rows. */
export interface CheckActions {
  /** Ouvrir: the viewer opens analysed files of their own folder only. */
  open: boolean;
  /** Voir: shows the file in the file manager; only a file of its own folder can be revealed. */
  reveal: boolean;
}

export interface CheckItem {
  key: string;
  entry: ExplorerEntry;
  reason: CheckReason;
  actions: CheckActions;
}

/** The pass stages the time is told by. */
export type TimePartId = "read" | "recognition" | "cut" | "meaning" | "write" | "inventory";

export interface TimePart {
  id: TimePartId;
  ms: number;
}

export type OverviewBlock =
  /** No folder at all: the first-start guidance, pointing at the left. */
  | { kind: "firstStart" }
  /** Nothing analysed yet: the three steps and where the user stands in them. */
  | { kind: "steps"; chosen: number; filesFound: number; running: boolean }
  /** Nothing analysed yet: what one family can read and how many of its files wait. */
  | {
      kind: "familyCard";
      family: Family;
      folder: string | null;
      folderChosen: boolean;
      folderState: FolderState;
      total: number;
      waiting: number;
      running: boolean;
    }
  | { kind: "figures"; families: FamilyFigures[] }
  /** The pass of a family failed before it could report anything. */
  | { kind: "passFailed"; family: Family; error: AppError }
  | {
      kind: "check";
      /** Every file to check: unreadable ones and the ones the last pass could not read. */
      total: number;
      items: CheckItem[];
      /** Not listed here. */
      more: number;
      /** What the state filter of the left list would show, for the link. */
      inList: number;
    }
  /** Files the pass dropped from the index because they are no longer in the folder. */
  | { kind: "removed"; family: Family; count: number }
  | { kind: "renames"; family: Family; items: RenamedFile[]; failed: number }
  | { kind: "ocr"; family: Family; files: string[] }
  | { kind: "unreadFormats"; family: Family; formats: UnreadFormat[]; total: number }
  | { kind: "knowledge"; family: Family; summary: KnowledgeSummary }
  | { kind: "capabilities"; codes: string[] }
  | { kind: "time"; family: Family; totalMs: number; parts: TimePart[] };

export type OverviewPhase = "firstStart" | "beforeAnalysis" | "report";

export interface OverviewPage {
  phase: OverviewPhase;
  blocks: OverviewBlock[];
}

function inputOf(input: OverviewInput, family: Family): FamilyReportInput {
  return family === "documents" ? input.documents : input.tables;
}

/** A path as the summaries report it, comparable with an entry's. */
function normal(path: string): string {
  return path.replace(/\\/g, "/");
}

/** Whether a family has been analysed, in this session or before: files already read, or a pass that
 * reported (even one that found nothing to read). */
function isAnalysed(counts: FamilyCounts, report: FamilyReportInput): boolean {
  return counts.analysed + counts.unreadable > 0 || report.summary !== null;
}

function outcomeOf(summary: IndexSummary): PassOutcome {
  if (summary.cancelled === true) {
    return "stopped";
  }
  return summary.indexedFiles === 0 && summary.emptyFiles.length + summary.failedFiles.length === 0
    ? "nothing"
    : "done";
}

/** Who can do what to a file of the list: the explorer's own gate (`isOpenable` for Ouvrir); Voir is
 * offered for every file, wherever it sits and whatever its state. */
export function checkActions(entry: ExplorerEntry): CheckActions {
  return { open: isOpenable(entry), reveal: true };
}

/** The view the link « Afficher les N dans la liste de gauche » sets: the files in the unreadable
 * state, in both families. The caller also clears the typed search. */
export function checkView(): ExplorerView {
  return viewShowingState("unreadable");
}

/**
 * Why a file is in the list. The last pass of its family says it when it ran this session (an error
 * code, or too little legible text). Otherwise the index says it: a document is unreadable only when
 * it was read and held no text, and the inventory records whether text recognition ran on it; a
 * workbook is unreadable when it holds no usable table. So no unreadable file is ever left without a
 * reason.
 */
function reasonOf(entry: ExplorerEntry, summary: IndexSummary | null): CheckReason {
  const path = normal(entryIndexKey(entry));
  const failed = summary?.failedFiles.find((file) => normal(file.path) === path);
  if (failed !== undefined) {
    return { kind: "error", code: failed.code, data: failed.data };
  }
  if (summary?.lowConfidenceFiles.some((file) => normal(file) === path) === true) {
    return { kind: "lowConfidence" };
  }
  if (entry.family === "tables") {
    return { kind: "noTable" };
  }
  return entry.record.indexMetadata?.ocrEngine != null ? { kind: "recognised" } : { kind: "empty" };
}

/**
 * The files that ask for a look: every unreadable file, wherever it sits (each folder's inventory
 * joins a file to what its own family made of it), and every file the last pass of its family failed
 * to read (the pass retries those, so the inventory still shows them as to analyse). Sorted by name.
 */
function filesToCheck(input: OverviewInput): CheckItem[] {
  const picked = new Map<string, ExplorerEntry>();
  for (const entry of input.entries) {
    if (entry.state === "unreadable") {
      picked.set(entry.key, entry);
    }
  }
  const byKey = new Map(input.entries.map((entry) => [entry.key, entry]));
  for (const family of FAMILIES) {
    const passFamily = family === "documents" ? "documents" : "data";
    for (const file of inputOf(input, family).summary?.failedFiles ?? []) {
      const entry = byKey.get(explorerKeyOfIndexKey(passFamily, normal(file.path)));
      if (entry !== undefined) {
        picked.set(entry.key, entry);
      }
    }
  }
  return sortEntries([...picked.values()], "name").map((entry) => ({
    key: entry.key,
    entry,
    reason: reasonOf(entry, inputOf(input, entry.family).summary),
    actions: checkActions(entry),
  }));
}

/** The time of a pass by stage, stages under half a second left out. */
function timeParts(summary: IndexSummary): TimePart[] {
  const timings = summary.timings;
  if (timings === null) {
    return [];
  }
  const parts: TimePart[] =
    timings.pass === "documents"
      ? [
          { id: "read", ms: timings.hashMs + timings.extractMs },
          { id: "recognition", ms: timings.ocrMs },
          { id: "cut", ms: timings.chunkMs },
          { id: "meaning", ms: timings.embedMs },
          { id: "write", ms: timings.writeMs },
        ]
      : [
          { id: "inventory", ms: timings.buildInventoryMs },
          { id: "write", ms: timings.writeMs },
        ];
  return parts.filter((part) => part.ms >= 500);
}

function figuresOf(
  family: Family,
  counts: FamilyCounts,
  report: FamilyReportInput,
): FamilyFigures {
  const summary = report.summary;
  return {
    family,
    folder: report.folder,
    folderState: report.folderState,
    running: report.running,
    analysed: counts.analysed,
    unreadable: counts.unreadable,
    toAnalyse: counts.toAnalyse,
    meaningPercent: report.meaningPercent,
    needsAnalyse: report.needsAnalyse,
    lastPass:
      summary === null
        ? null
        : { at: report.finishedAt, durationMs: report.durationMs, outcome: outcomeOf(summary) },
  };
}

/** A block for each family whose pass failed before it could report anything. */
function failureBlocks(input: OverviewInput): OverviewBlock[] {
  return FAMILIES.flatMap((family): OverviewBlock[] => {
    const report = inputOf(input, family);
    return report.error !== null && report.summary === null
      ? [{ kind: "passFailed", family, error: report.error }]
      : [];
  });
}

/** What the last passes added beyond the counts, one family after the other inside each kind. */
function reportBlocks(input: OverviewInput): OverviewBlock[] {
  const blocks: OverviewBlock[] = [];
  const summaries = FAMILIES.flatMap((family) => {
    const summary = inputOf(input, family).summary;
    return summary === null ? [] : [{ family, summary }];
  });
  for (const { family, summary } of summaries) {
    if (summary.removedFiles.length > 0) {
      blocks.push({ kind: "removed", family, count: summary.removedFiles.length });
    }
  }
  for (const { family, summary } of summaries) {
    if (summary.renamedFiles.length > 0 || summary.renameFailedFiles.length > 0) {
      blocks.push({
        kind: "renames",
        family,
        items: summary.renamedFiles,
        failed: summary.renameFailedFiles.length,
      });
    }
  }
  for (const { family, summary } of summaries) {
    if (summary.ocrFiles.length > 0) {
      blocks.push({ kind: "ocr", family, files: summary.ocrFiles });
    }
  }
  for (const { family, summary } of summaries) {
    if (summary.unreadFormats.length > 0) {
      blocks.push({
        kind: "unreadFormats",
        family,
        formats: summary.unreadFormats,
        total: summary.unreadFormats.reduce((sum, format) => sum + format.count, 0),
      });
    }
  }
  for (const { family, summary } of summaries) {
    if (summary.knowledge != null) {
      blocks.push({ kind: "knowledge", family, summary: summary.knowledge });
    }
  }
  const codes = [
    ...new Set(summaries.flatMap(({ summary }) => summary.unavailableCapabilities)),
  ].filter((code) => code in CAPABILITY_KEYS);
  if (codes.length > 0) {
    blocks.push({ kind: "capabilities", codes });
  }
  for (const { family, summary } of summaries) {
    const report = inputOf(input, family);
    const totalMs = report.durationMs ?? summary.timings?.totalMs ?? 0;
    if (totalMs > 0) {
      blocks.push({ kind: "time", family, totalMs, parts: timeParts(summary) });
    }
  }
  return blocks;
}

/**
 * The page, from what the two folders hold and what the last pass of each family said.
 *
 * - No folder chosen at all: the first-start guidance.
 * - Folders chosen, nothing analysed in either: the three steps, then a card per family.
 * - Otherwise the report: figures per family, the files to check, then what the passes added.
 */
export function overviewModel(input: OverviewInput): OverviewPage {
  if (input.documents.folder === null && input.tables.folder === null) {
    return { phase: "firstStart", blocks: [{ kind: "firstStart" }] };
  }
  const counts = countsByFamily(input.entries);
  const chosen = FAMILIES.filter((family) => inputOf(input, family).folder !== null);

  if (!FAMILIES.some((family) => isAnalysed(counts[family], inputOf(input, family)))) {
    return {
      phase: "beforeAnalysis",
      blocks: [
        {
          kind: "steps",
          chosen: chosen.length,
          filesFound: counts.documents.total + counts.tables.total,
          running: input.documents.running || input.tables.running,
        },
        ...failureBlocks(input),
        ...FAMILIES.map((family): OverviewBlock => {
          const report = inputOf(input, family);
          return {
            kind: "familyCard",
            family,
            folder: report.folder,
            folderChosen: report.folder !== null,
            folderState: report.folderState,
            total: counts[family].total,
            waiting: counts[family].analysable,
            running: report.running,
          };
        }),
      ],
    };
  }

  const blocks: OverviewBlock[] = [
    {
      kind: "figures",
      families: chosen.map((family) => figuresOf(family, counts[family], inputOf(input, family))),
    },
  ];
  blocks.push(...failureBlocks(input));
  const items = filesToCheck(input);
  if (items.length > 0) {
    blocks.push({
      kind: "check",
      total: items.length,
      items: items.slice(0, MAX_CHECK_LISTED),
      more: Math.max(0, items.length - MAX_CHECK_LISTED),
      inList: input.entries.filter((entry) => entry.state === "unreadable").length,
    });
  }
  blocks.push(...reportBlocks(input));
  return { phase: "report", blocks };
}
