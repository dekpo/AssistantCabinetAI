import type { FillPlan, FillPreview, FillReport, FillRequest } from "./fill";
import { Channel, invoke } from "@tauri-apps/api/core";

/**
 * The only bridge to the outside. The webview never calls the gateway itself: the address, the
 * context cap, the path allow-list and the stored settings all live behind these commands, in
 * Rust, where a change to the view cannot loosen them.
 */

export type ThemeChoice = "light" | "dark" | "system";

export interface AppSettings {
  /** Chosen locale, or null on a machine where nothing has been chosen yet. */
  locale: string | null;
  theme: ThemeChoice;
  serverUrl: string;
  modelAlias: string;
  embeddingAlias: string;
  workFolder: string | null;
  /** The Data Folder (CSV/XLSX), a sibling of `workFolder` rather than a second copy of it: the
   * two are chosen independently and validated through the same allow-list. */
  dataFolder: string | null;
  /** Seconds of silence before an answer is abandoned. Silence, not duration: an answer that
   * keeps arriving is never cut off, however long it takes. Rust clamps whatever is sent. */
  answerIdleTimeoutSeconds: number;
  /** The subfolder of the documents folder where generated letters are written. Rust keeps it one
   * clean name, whatever is sent. */
  generatedFolderName: string;
  /** Reveals the developer panel that shows each question's timings. Nothing renders it yet; never
   * shown in normal chat. Off by default. */
  showDiagnostics: boolean;
  /** Appends numbers and machine codes, never a question or a file name, to
   * `retrieval-timings.jsonl` and `analysis-timings.jsonl` in the app-data folder. Off by default;
   * edited in `settings.json` until the settings dialog has its "Advanced" group. */
  writeTimingLog: boolean;
}

export interface AppSnapshot {
  settings: AppSettings;
  /** What the operating system reports. The catalogue registry decides whether it is usable. */
  systemLocale: string;
  /** Where `settings.json` lives on this machine. Shown in the settings panel, never guessed. */
  settingsPath: string;
  /** `~/AssistantCabinetAI/Docs`, proposed when nothing has been chosen. Null when there is no home. */
  suggestedWorkFolder: string | null;
  /** `~/AssistantCabinetAI/Data`, a sibling of `suggestedWorkFolder` rather than a second Documents
   * Folder. Null when there is no home. */
  suggestedDataFolder: string | null;
  /** Machine codes for what went wrong while reading, without preventing the window opening. */
  warnings: string[];
}

export interface HealthSnapshot {
  status: "ok" | "degraded";
  providerReachable: boolean;
  aliases: string[];
  defaultModelAlias: string;
  embeddingAlias: string;
  issues: string[];
  defaultOutputLocale: string;
  outputLocales: string[];
  /** Each chat alias's context window, in tokens. Rust reads it to fit the conversation's memory;
   * the view has no use for it beyond display. */
  contextWindows: Record<string, number>;
  maxOutputTokens: number | null;
}

export interface ChatTurn {
  role: "user" | "assistant";
  content: string;
}

export type PageOrigin = "textLayer" | "ocr";

export interface Evidence {
  chunkId: string;
  relativePath: string;
  pageNumber: number;
  section: number;
  text: string;
  score: number;
  origin: PageOrigin;
  confidence: number | null;
}

/** What the document pipeline makes of a filesystem object. Machine codes: the catalogue writes
 * the word, in the practice's language. */
export type FileKind =
  | "document_text"
  | "document_pdf"
  | "document_office"
  | "image"
  | "tabular_candidate"
  | "unsupported"
  | "other";

export type Readability = "readable" | "unreadable" | "not_assessed";

export type ProcessingStatus = "discovered" | "pending" | "processing" | "indexed" | "failed";

/** `recognised_ocr` is not a flavour of `native_text`: what a machine read from a picture is not
 * what the file itself carries, and the interface says so. */
export type ExtractionMethod = "native_text" | "recognised_ocr" | "none";

export interface IndexMetadata {
  indexedSha256: string;
  chunkCount: number;
  ocrEngine: string | null;
  ocrEngineVersion: string | null;
}

/**
 * One physical file in the work folder. Filesystem truth, joined with what the local index made
 * of it (`docs/WORK-FOLDER-INVENTORY.md`). Never carries document text.
 */
export interface FileRecord {
  id: string;
  relativePath: string;
  name: string;
  stem: string;
  /** Lowercase, without the dot. Read from the filesystem, never inferred from content. */
  extension: string;
  kind: FileKind;
  mimeType: string;
  sizeBytes: number;
  modifiedAt: number | null;
  sha256: string | null;
  readability: Readability;
  processingStatus: ProcessingStatus;
  extractionMethod: ExtractionMethod;
  indexed: boolean;
  indexMetadata: IndexMetadata | null;
}

export interface FolderNode {
  name: string;
  relativePath: string;
  folders: FolderNode[];
  files: string[];
}

export interface InventorySummary {
  totalFiles: number;
  indexedFiles: number;
  unreadableFiles: number;
  notAssessedFiles: number;
  folderCount: number;
  byExtension: Record<string, number>;
  byKind: Record<string, number>;
}

export interface InventoryReport {
  rootIdentifier: string;
  summary: InventorySummary;
  files: FileRecord[];
  hierarchy: FolderNode;
}

/**
 * A question the work folder answered by itself, with no gateway call. Rust sends the facts and
 * a machine code; this side writes the sentence (`docs/LANGUAGE-AND-LOCALE.md`).
 */
export type FolderAnswer =
  | { kind: "file_count"; total: number }
  | { kind: "file_list"; files: FileRecord[] }
  | { kind: "extension_count"; extension: string; count: number }
  | { kind: "extension_list"; extension: string; files: FileRecord[] }
  | { kind: "image_list"; files: FileRecord[] }
  | { kind: "unreadable_count"; count: number }
  | { kind: "unreadable_list"; files: FileRecord[] }
  | { kind: "indexed_count"; count: number }
  | { kind: "indexed_list"; files: FileRecord[] }
  | { kind: "folder_list"; folders: string[] }
  | { kind: "folder_tree"; root: string; lines: string[] }
  | { kind: "file_details"; file: FileRecord }
  | { kind: "ambiguous_reference"; query: string; candidates: FileRecord[] }
  | { kind: "no_matching_file"; query: string }
  | { kind: "file_unreadable"; file: FileRecord }
  /** A file the folder holds but this conversation's selection does not. */
  | { kind: "file_not_selected"; query: string }
  /** No document is selected, and the question was about the documents themselves. */
  | { kind: "nothing_selected" };

/**
 * How much of the work folder an answer really rests on. Computed in Rust from the evidence it
 * assembled and the inventory's own counts, never asked of the model - which only knows what it
 * was sent (`docs/WORK-FOLDER-INVENTORY.md`). Present only for a question that asked about every
 * document, because that is the only shape that claims completeness.
 */
export interface EvidenceCoverage {
  filesCovered: number;
  indexedFiles: number;
  unreadableFiles: number;
}

export type ChatStreamEvent =
  | { event: "delta"; text: string }
  | { event: "completed"; text: string }
  | { event: "sources"; sources: Evidence[]; coverage: EvidenceCoverage | null }
  | { event: "folder_answer"; answer: FolderAnswer };

/**
 * How far one analysis pass has got. Counts only: no document text and not even a file name
 * crosses this channel, so a progress bar cannot become a second place document content leaks to.
 */
export interface IndexProgress {
  processedFiles: number;
  totalFiles: number;
  /** Inside the file being embedded: batches done and batches in all. Both 0 outside that. */
  batchIndex: number;
  batchTotal: number;
}

/** A file that could not be embedded: the pass went on without it and will try it again. */
export interface FailedFile {
  path: string;
  /** A machine code, localised by `errorMessage`. */
  code: string;
  data: Record<string, unknown>;
}

export interface RenamedFile {
  from: string;
  to: string;
}

export interface IndexSummary {
  scannedFiles: number;
  indexedFiles: number;
  unchangedFiles: number;
  emptyFiles: string[];
  ocrFiles: string[];
  lowConfidenceFiles: string[];
  /** Documents dropped from the index because they are no longer in the work folder. */
  removedFiles: string[];
  /** Files renamed to a clean name (no spaces or accents) before the pass read anything. Names
   * only, relative to the work folder. */
  renamedFiles: RenamedFile[];
  /** Files that needed a clean name and could not be renamed. Left as they were. */
  renameFailedFiles: string[];
  /** Files that could not be embedded. Never recorded as analysed; the next pass retries them. */
  failedFiles: FailedFile[];
  /** Machine codes for an ingestion capability that did not start. Empty on a healthy install. */
  unavailableCapabilities: string[];
  chunkCount: number;
  /** Where the pass spent its time. Numbers only. */
  timings: AnalysisTimings | null;
}

export type AnalysisPass = "documents" | "data";

/** One of the slowest files of a pass, by position in the pass. Never a name. */
export interface FileTiming {
  position: number;
  totalMs: number;
  extractMs: number;
  ocrMs: number;
  chunkMs: number;
  embedMs: number;
  writeMs: number;
  chunks: number;
}

/**
 * The timings of one Analyse pass (`knowledge::diagnostics` in Rust). The Documents pass fills the
 * hash, extract, OCR, chunk, embed and write stages; the Data pass fills `buildInventoryMs` and
 * `writeMs`. `extractMs` excludes `ocrMs`.
 */
export interface AnalysisTimings {
  pass: AnalysisPass;
  filesScanned: number;
  /** Files read in this pass: not skipped as unchanged. */
  filesProcessed: number;
  hashMs: number;
  extractMs: number;
  ocrMs: number;
  chunkMs: number;
  embedMs: number;
  embedBatches: number;
  embeddedChars: number;
  writeMs: number;
  buildInventoryMs: number;
  chunksTotal: number;
  /** Chunks whose text is byte-identical to an earlier chunk of the same pass. */
  chunksRepeatingEarlierText: number;
  totalMs: number;
  slowestFiles: FileTiming[];
}

/**
 * The Data Folder as its card shows it: every file, each with its tabular state carried in the
 * same `readability` / `processingStatus` fields the Documents Folder listing reads - `indexed` is
 * green (a usable table), `failed` is red (analysed, nothing usable, or not a spreadsheet at all),
 * `discovered` and `pending` are orange (not analysed yet, or changed since).
 */
export interface DataFolderReport {
  rootIdentifier: string;
  summary: DataFolderSummary;
  files: FileRecord[];
}

export interface DataFolderSummary {
  totalFiles: number;
  analysedFiles: number;
  unreadableFiles: number;
  notAssessedFiles: number;
}

/** Why the tabular engine could not answer exactly. Machine codes: this side writes the sentence. */
export type NotAnswerableReason =
  | "sheet_not_found"
  | "ambiguous_sheet_name"
  | "column_not_found"
  | "ambiguous_column_name"
  | "row_out_of_range"
  | "non_numeric_column"
  | "formula_cannot_be_verified"
  | "empty_sheet"
  | "filter_not_supported"
  | "non_date_column"
  | "value_not_found"
  | "group_threshold_not_supported";

/** One filter a value was actually computed under, exactly as `tabular::engine` ran it - data,
 * never prose (`docs/DECISIONS.md`, session 11): this side writes "fournisseur = Alpha" or "date
 * in March 2026" from it, in her language, for the "Understood as" line under a filtered answer. */
export type AppliedFilter =
  | { kind: "equals"; column: string; value: string }
  | { kind: "contains"; column: string; value: string }
  | { kind: "greater_than"; column: string; threshold: number }
  | { kind: "less_than"; column: string; threshold: number }
  | { kind: "between"; column: string; low: number; high: number }
  /** ISO weekday: 1 = Monday .. 7 = Sunday. */
  | { kind: "weekday"; column: string; weekday: number }
  | { kind: "month"; column: string; month: number }
  | { kind: "year"; column: string; year: number }
  /** Inclusive both ends, ISO 8601 (`yyyy-mm-dd`). */
  | { kind: "date_range"; column: string; start: string; end: string }
  | { kind: "in"; column: string; values: string[] };

/** Where inside a workbook a value came from: the tabular sibling of `Evidence.pageNumber`. */
export interface TabularLocator {
  sheet: string;
  headerRow: number | null;
  column: string | null;
  /** 0-indexed, inclusive, into the sheet's data rows. */
  rowRange: [number, number] | null;
  /** Every filter this value was actually computed under - "what was understood"
   * (`docs/DECISIONS.md`, session 11). Empty for an unfiltered value. */
  filters: AppliedFilter[];
}

export interface TabularCell {
  column: string;
  text: string;
}

export interface TabularRow {
  rowIndex: number;
  cells: TabularCell[];
}

/** A sum, minimum or maximum together with what its column carries beside the number: the unit
 * read from its cells (`€`, `$`, `%`), when every cell that had one agreed, and how many
 * non-empty cells could not be read as a number - never folded into `value`, always shown beside
 * it (`docs/DECISIONS.md`, D2). */
export interface NumericAggregate {
  value: number;
  unit: string | null;
  unparsed: number;
}

export type TabularValue =
  | { kind: "count"; value: number }
  | { kind: "sum"; value: NumericAggregate }
  | { kind: "min"; value: NumericAggregate }
  | { kind: "max"; value: NumericAggregate }
  /** The arithmetic mean, over the same formula-free numeric cells a `sum` would read
   * (`docs/DECISIONS.md`, D5). Never labelled a total. */
  | { kind: "mean"; value: NumericAggregate }
  /** The middle value of the full sorted column, never a shortcut over group medians. */
  | { kind: "median"; value: NumericAggregate }
  | { kind: "distinct"; value: string[] }
  | { kind: "group_sums"; value: { group: string; sum: number }[] }
  /** The groups with the largest totals, largest first: a total per group, never one row. */
  | {
      kind: "largest_group";
      value: { groupColumn: string; top: { group: string; sum: number }[]; groupCount: number };
    }
  /** The mirror of `largest_group`: smallest total first, never the smallest single row. */
  | {
      kind: "least_group";
      value: { groupColumn: string; top: { group: string; sum: number }[]; groupCount: number };
    }
  /** The top N groups by total, N capped at 50 and the cap reported (`docs/DECISIONS.md`, D5). */
  | {
      kind: "top_groups";
      value: {
        groupColumn: string;
        top: { group: string; sum: number }[];
        groupCount: number;
        /** What she actually asked for, uncapped - `top.length` is at most 50. */
        requested: number;
        capped: boolean;
      };
    }
  /** The groups whose total is above or below a threshold, with the nearest group when none is. */
  | {
      kind: "groups_beyond";
      value: {
        groupColumn: string;
        threshold: number;
        above: boolean;
        matches: { group: string; sum: number }[];
        groupCount: number;
        extreme: { group: string; sum: number };
      };
    }
  /** Rows per group, largest first - a count, never a sum. */
  | { kind: "count_per_group"; value: { group: string; count: number }[] }
  | { kind: "largest_row"; value: TabularRow }
  | { kind: "row"; value: TabularRow }
  | { kind: "rows"; value: TabularRow[] };

export type TabularDerivation =
  | { kind: "computed"; operation: string; row_count: number }
  /** Session 14's hidden interpreter (`docs/SESSION-DATA-14-Query-Plan.md`): the engine still did
   * every calculation, over a full pass, exactly as `computed` describes - the only difference is
   * that a model, reading the workbook's schema alone, wrote the plan that chose this operation,
   * validated by Rust before the engine ever ran it. `plan` is that validated plan, kept so the
   * same question on the same file can be answered again without asking the model a second time.
   * Rust's field names, snake_case, as they cross - unlike most of this file's camelCase wire
   * shapes, this mirrors `tabular::engine::TabularDerivation`'s own untouched field casing. */
  | {
      kind: "interpreted_by_model";
      operation: string;
      row_count: number;
      model_alias: string;
      plan: unknown;
    };

/** A structural fact read from a workbook's inventory. Rust's field names, as they cross. */
export type StructuralAnswer =
  | { kind: "sheet_names"; sheets: string[] }
  | { kind: "row_count"; sheet: string; rows: number }
  | { kind: "column_names"; sheet: string; columns: string[] }
  | { kind: "is_column_numeric"; sheet: string; column: string; numeric: boolean }
  | { kind: "has_formulas"; sheet: string; has_formulas: boolean };

/**
 * A question the tabular engine answered, with no model and no gateway call: tables were
 * selected and no document was (`docs/SELECTION-AND-MEMORY.md`, tier 2). Facts and machine codes;
 * the sentence is written in `lib/tabularAnswer.ts`.
 */
export type TabularAnswer =
  | {
      kind: "value";
      file: string;
      value: TabularValue;
      locator: TabularLocator;
      derivation: TabularDerivation;
      /** Set when the model was asked ("Ask AI"), could not do better, and this computed value was kept. */
      modelAttempt?: { modelAlias: string; durationMs: number } | null;
    }
  | { kind: "structural"; file: string; answer: StructuralAnswer }
  | {
      kind: "nudge";
      file: string;
      /** Null when the question was not recognised at all. */
      reason: NotAnswerableReason | null;
      availableSheets: string[];
      availableColumns: string[];
      /** A column the engine can total as it stands, for a concrete example. */
      exampleColumn: string | null;
      /** A text column to group by, for a second example. */
      exampleGroup: string | null;
      /** Set only when `reason` is `filter_not_supported`: the column a residual question word
       * was found in, when one was. */
      filterColumn: string | null;
      /** Set when `reason` is `filter_not_supported` or `value_not_found`: the word from the
       * question that named real data, or looked like an attempt to, exactly as typed. */
      filterValue: string | null;
      /** Set only when `reason` is `value_not_found`: up to five real values close to
       * `filterValue`, so she can see what the column actually holds. Empty otherwise. */
      closeValues: string[];
      /** Set only when session 14's hidden interpreter actually tried the model for this
       * question, whatever it came to - a wrong plan, invalid JSON, or a timeout. `null` for a
       * nudge the classifier gave without ever asking the model: that one really did cost
       * nothing, and must not claim otherwise. */
      modelAttempt: { modelAlias: string; durationMs: number } | null;
    }
  /** A group question with several columns it could total, none named: asked, never picked. */
  | { kind: "which_measure"; file: string; groupColumn: string; candidates: string[] }
  /** A residual word named real data in more than one reachable column: asked, never picked. */
  | { kind: "which_column"; file: string; value: string; candidates: string[] }
  | { kind: "workbook_unreadable"; file: string }
  | { kind: "workbook_not_analysed"; file: string }
  | { kind: "which_workbook"; candidates: string[] }
  | { kind: "ambiguous_reference"; query: string; candidates: string[] }
  | { kind: "file_not_selected"; query: string }
  | { kind: "no_matching_file"; query: string }
  | { kind: "no_usable_table" };

/** Why one side of a mixed answer has nothing to show (`docs/SESSION-DATA-16-Mixed-Tier.md`,
 * mechanism 6: partial refusal). */
export type MixedPartUnavailable =
  | "not_asked_about"
  | "no_evidence"
  | "gateway_unavailable"
  | "not_linked";

/** One number the model wrote that matched neither the table's own value nor any excerpt's text
 * verbatim - appended, never silently rewritten into the answer. */
export interface NumericCorrection {
  /** The number exactly as the model wrote it. */
  claimed: string;
  /** The table's real value, raw - the interface formats it in her language. */
  correct: number;
}

/** Documents and tables both selected, and the question was neither clearly data-only nor a pure
 * refusal: the mixed tier decomposed it, computed and cited across both sides, and checked the
 * model's prose against what it was actually given. */
export interface MixedAnswer {
  /** The model's prose. Empty when generation was never attempted - the table answered alone,
   * with no gateway call, because the document side had nothing to add. */
  answer: string;
  /** The tabular part's own computed value, when the question had one. */
  table: TabularAnswer | null;
  tableUnavailable: MixedPartUnavailable | null;
  /** The document excerpts the model was actually sent. */
  documentSources: Evidence[];
  documentsUnavailable: MixedPartUnavailable | null;
  /** Appended, never silent: a number the model wrote that did not match the table or any
   * excerpt. */
  corrections: NumericCorrection[];
  /** A bracketed citation (`"[3]"`) the model wrote that does not resolve to any of
   * `documentSources`. */
  rejectedCitations: string[];
  /** Figures the model stated that are in none of the excerpts and not in the question, when no table figure is shown. */
  unverifiedNumbers: string[];
}

export interface AskAnswer {
  answer: string;
  sources: Evidence[];
  /** Set when the work folder answered the question itself. `answer` is then empty. */
  folderAnswer: FolderAnswer | null;
  /** Set when the question asked about every document. */
  coverage: EvidenceCoverage | null;
  /** Documents this answer could not have used: never analysed, or changed since they were. */
  unanalysedFiles: number;
  /** Files the conversation's scope named that are gone or have changed since they were chosen.
   * They were left out of the answer. */
  scopeOutdated: string[];
  /** She selected no document, so this answer rests on none. Said under the answer. */
  withoutDocuments: boolean;
  /** Figures the answer states that appear in none of its excerpts and not in the question. */
  unverifiedNumbers: string[];
  /** A letter was asked for and the template and the rows it needs were found: a plan to approve.
   * `answer` is then empty. */
  fillPlan: FillPlan | null;
  /** Set when tables were selected and no document: the tabular engine answered it. `answer` is
   * then empty. */
  tabularAnswer: TabularAnswer | null;
  /** Documents and tables were both selected, but this question was clearly and only about the
   * data, so the tabular engine answered it alone and the selected documents were never read. */
  documentsNotNeeded: boolean;
  /** Documents and tables were both selected, and the mixed tier answered across both
   * (`docs/SESSION-DATA-16-Mixed-Tier.md`). */
  mixedAnswer: MixedAnswer | null;
  /** Where this question spent its time. Numbers and machine codes only. */
  diagnostics: RetrievalDiagnostics | null;
}

export type RetrievalPath =
  | "documents"
  | "tabular"
  | "mixed"
  | "folder_answer"
  | "without_documents";

export type RetrievalPlan = "one_file" | "every_document" | "whole_folder";

/**
 * The timings and counts of one question (`knowledge::diagnostics` in Rust). Every `...Ms` is the
 * wall time of one stage, and stays 0 when the question did not go through it. Nothing renders
 * this yet; `AppSettings.showDiagnostics` is reserved for the panel that will.
 */
export interface RetrievalDiagnostics {
  path: RetrievalPath;
  plan: RetrievalPlan | null;
  selectedDocuments: number;
  selectedWorkbooks: number;
  chunksConsidered: number;
  chunksSelected: number;
  estimatedInputChars: number;
  inventoryMs: number;
  scopeMs: number;
  routingMs: number;
  embeddingMs: number;
  searchMs: number;
  workbookLoadMs: number;
  workbookCacheHit: boolean;
  generationFirstTokenMs: number | null;
  generationTotalMs: number;
  totalMs: number;
}

/**
 * Which files a conversation is about (`AnalysisScope` in Rust): the whole folder, or the files she
 * ticked. An explicit selection with no entries is "no document", the default, answered without
 * excerpts (`docs/SELECTION-AND-MEMORY.md`). Selecting only ever picks among files the work folder
 * already holds. Timestamps are milliseconds since the epoch, by the workstation clock.
 */
export type ScopeMode =
  | { kind: "whole_folder" }
  | { kind: "explicit"; entries: ScopeEntry[] };

export interface ScopeEntry {
  /** The handle into the work folder. Never an absolute path. */
  relativePath: string;
  /** The file's `id` when it was chosen, so a file replaced mid-conversation is noticed. */
  pinnedId: string;
  addedAt: number;
}

/**
 * One selection list, as a card holds it. Both cards use this same shape, each relative to its own
 * folder; `combineScopes` joins them into what is sent with a question.
 */
export interface AnalysisScope {
  mode: ScopeMode;
  createdAt: number;
  updatedAt: number;
}

/** What a question is sent with: the Documents selection (`mode`) and the Data selection
 * (`dataMode`), built from both lists before every question. */
export interface CombinedScope extends AnalysisScope {
  dataMode: ScopeMode;
}

export function loadAppSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("load_app_snapshot");
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_settings", { settings });
}

/**
 * Put every setting back to a first launch, the documents folder included, and return what was
 * stored. The defaults come from Rust rather than from here: a default server address can come
 * from the environment, so it is read, never assumed.
 *
 * The local index is untouched - documents stay analysed, and choosing the same folder again
 * finds them. Emptying the index is `resetIndex`, in the folder card.
 */
export function resetSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("reset_settings");
}

/** Opens the system folder dialog and validates the choice. Returns the accepted path. */
export function chooseWorkFolder(): Promise<string> {
  return invoke<string>("choose_work_folder");
}

/** Create `~/AssistantCabinetAI/Docs` if it is missing, then return the accepted path. */
export function ensureSuggestedWorkFolder(): Promise<string> {
  return invoke<string>("ensure_suggested_work_folder");
}

/**
 * Show the work folder in the system's own file manager: Explorer on Windows, Finder on macOS.
 * No path crosses the bridge - Rust reads the folder from settings, so this view can ask for
 * "the work folder" and never for a folder of its own choosing.
 */
export function revealWorkFolder(): Promise<void> {
  return invoke<void>("reveal_work_folder");
}

/** The Data Folder equivalent of `chooseWorkFolder`. Opens the system folder dialog and
 * validates the choice through the same allow-list. Returns the accepted path. */
export function chooseDataFolder(): Promise<string> {
  return invoke<string>("choose_data_folder");
}

/** Create `~/AssistantCabinetAI/Data` if it is missing, then return the accepted path. */
export function ensureSuggestedDataFolder(): Promise<string> {
  return invoke<string>("ensure_suggested_data_folder");
}

/** The Data Folder equivalent of `revealWorkFolder`. No path crosses the bridge: Rust reads the
 * folder from settings. */
export function revealDataFolder(): Promise<void> {
  return invoke<void>("reveal_data_folder");
}

/** Show one file of the Data Folder, selected in the system's file manager. A path relative to
 * the Data Folder, as its listing gives it; Rust refuses anything else. */
export function revealDataFile(relativePath: string): Promise<void> {
  return invoke<void>("reveal_data_file", { relativePath });
}

/**
 * The Data Folder's Analyse: clean file names, then every CSV/XLSX parsed on this computer. No
 * embedding and no AI: it is over in moments. Reported in the Documents pass's own shape -
 * `indexedFiles` counts the workbooks holding a usable table.
 */
export function indexDataFolder(
  onProgress?: (progress: IndexProgress) => void,
): Promise<IndexSummary> {
  const channel = new Channel<IndexProgress>();
  channel.onmessage = (message) => onProgress?.(message);
  return invoke<IndexSummary>("index_data_folder", { onProgress: channel });
}

/** What is in the Data Folder right now, and which workbooks hold a usable table. */
export function dataFolderInventory(): Promise<DataFolderReport> {
  return invoke<DataFolderReport>("data_folder_inventory");
}

/** Forget every workbook analysis. The documents' analysis and every file stay as they are.
 * Confirm before calling it. */
export function resetDataIndex(): Promise<void> {
  return invoke<void>("reset_data_index");
}

/** The preview of a mail merge for the choices made on the plan card. */
export function fillPreview(request: FillRequest): Promise<FillPreview> {
  return invoke<FillPreview>("fill_preview", { request });
}

/** Writes the letters she approved into the `Generated` subfolder of the documents folder: new files
 * only, the template and the data untouched. */
export function fillGenerate(request: FillRequest): Promise<FillReport> {
  return invoke<FillReport>("fill_generate", { request });
}

/** Show one file of the work folder, selected in the system's file manager. Takes the path
 * relative to the folder, as the inventory lists it; Rust refuses anything else. */
export function revealWorkFile(relativePath: string): Promise<void> {
  return invoke<void>("reveal_work_file", { relativePath });
}

/**
 * Forget everything the index holds. Every document stays exactly where it is on disk: this
 * undoes the analysis, never the folder. Confirm before calling it.
 */
export function resetIndex(): Promise<void> {
  return invoke<void>("reset_index");
}

export function checkServerHealth(): Promise<HealthSnapshot> {
  return invoke<HealthSnapshot>("check_server_health");
}

/**
 * Stop the question being worked on, wherever it has got to: embedding it, searching the index, or
 * streaming the answer. The command returns at once; the stopped request then rejects with
 * `chat_cancelled`, which is how the interface learns it really ended.
 */
export function cancelChat(): Promise<void> {
  return invoke<void>("cancel_chat");
}

/**
 * One pass of discovery, extraction, chunking and embedding over the work folder.
 *
 * `onProgress` is called as the pass walks the folder, so a long analysis can show how far it has
 * got rather than only that it is busy.
 */
export function indexWorkFolder(
  onProgress?: (progress: IndexProgress) => void,
): Promise<IndexSummary> {
  const channel = new Channel<IndexProgress>();
  channel.onmessage = (message) => onProgress?.(message);
  return invoke<IndexSummary>("index_work_folder", { onProgress: channel });
}

/** Whether the local index has anything to search yet - a work folder can be chosen but never
 * analysed. Used to say so instantly, before spending a round trip on a question retrieval is
 * certain to refuse. */
export function hasIndexedDocuments(): Promise<boolean> {
  return invoke<boolean>("has_indexed_documents");
}

/**
 * Every question goes through here. Retrieval, then a sourced chat answer, rejecting with
 * `insufficient_evidence` rather than answering when the local index carries nothing relevant; or,
 * with no document selected, an answer without excerpts that says so. Rust holds the server
 * address, the model alias and the output locale, so the view cannot send a request elsewhere.
 */
export function askWithSources(
  question: string,
  onDelta: (text: string) => void,
  onSources: (sources: Evidence[], coverage: EvidenceCoverage | null) => void,
  onFolderAnswer?: (answer: FolderAnswer) => void,
  /** Ask the model even when the work folder could answer on its own. Her choice, made on an
   * answer she has already seen. */
  skipDeterministic = false,
  /** The files the question may draw on, from both folders. Left out, the whole documents
   * folder and no table. */
  scope?: CombinedScope,
  /** The conversation so far. Rust keeps as much of it as the chosen model can read. */
  history: ChatTurn[] = [],
): Promise<AskAnswer> {
  const channel = new Channel<ChatStreamEvent>();
  channel.onmessage = (message) => {
    if (message.event === "delta") {
      onDelta(message.text);
    } else if (message.event === "sources") {
      // Read from the event rather than from the resolved answer, because a stopped answer never
      // resolves and still has to show where its text came from.
      onSources(message.sources, message.coverage);
    } else if (message.event === "folder_answer") {
      onFolderAnswer?.(message.answer);
    }
  };
  return invoke<AskAnswer>("ask_with_sources", {
    question,
    skipDeterministic,
    scope,
    history,
    onEvent: channel,
  });
}

/**
 * What is in the work folder right now: counts, files and the folder hierarchy, read from the
 * disk and from the local index. Never from the model, so the panel and an answer about the
 * folder cannot disagree (`docs/WORK-FOLDER-INVENTORY.md`).
 */
export function workFolderInventory(): Promise<InventoryReport> {
  return invoke<InventoryReport>("work_folder_inventory");
}
