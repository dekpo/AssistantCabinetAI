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
  /** Machine codes for an ingestion capability that did not start. Empty on a healthy install. */
  unavailableCapabilities: string[];
  chunkCount: number;
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
  | "filter_not_supported";

/** Where inside a workbook a value came from: the tabular sibling of `Evidence.pageNumber`. */
export interface TabularLocator {
  sheet: string;
  headerRow: number | null;
  column: string | null;
  /** 0-indexed, inclusive, into the sheet's data rows. */
  rowRange: [number, number] | null;
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
  | { kind: "distinct"; value: string[] }
  | { kind: "group_sums"; value: { group: string; sum: number }[] }
  /** The groups with the largest totals, largest first: a total per group, never one row. */
  | {
      kind: "largest_group";
      value: { groupColumn: string; top: { group: string; sum: number }[]; groupCount: number };
    }
  | { kind: "largest_row"; value: TabularRow }
  | { kind: "row"; value: TabularRow }
  | { kind: "rows"; value: TabularRow[] };

export interface TabularDerivation {
  kind: "computed";
  operation: string;
  row_count: number;
}

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
      /** Set only when `reason` is `filter_not_supported`: the word from the question that named
       * real data, exactly as typed. */
      filterValue: string | null;
    }
  /** A group question with several columns it could total, none named: asked, never picked. */
  | { kind: "which_measure"; file: string; groupColumn: string; candidates: string[] }
  | { kind: "workbook_unreadable"; file: string }
  | { kind: "workbook_not_analysed"; file: string }
  | { kind: "which_workbook"; candidates: string[] }
  | { kind: "ambiguous_reference"; query: string; candidates: string[] }
  | { kind: "file_not_selected"; query: string }
  | { kind: "no_matching_file"; query: string }
  | { kind: "no_usable_table" };

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
  /** Set when tables were selected and no document: the tabular engine answered it. `answer` is
   * then empty. */
  tabularAnswer: TabularAnswer | null;
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
