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
  /** `~/AssistantCabinetAI`, proposed when nothing has been chosen. Null when there is no home. */
  suggestedWorkFolder: string | null;
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
  | { kind: "file_unreadable"; file: FileRecord };

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

export interface IndexSummary {
  scannedFiles: number;
  indexedFiles: number;
  unchangedFiles: number;
  emptyFiles: string[];
  ocrFiles: string[];
  lowConfidenceFiles: string[];
  chunkCount: number;
}

export interface AskAnswer {
  answer: string;
  sources: Evidence[];
  /** Set when the work folder answered the question itself. `answer` is then empty. */
  folderAnswer: FolderAnswer | null;
  /** Set when the question asked about every document. */
  coverage: EvidenceCoverage | null;
}

export function loadAppSnapshot(): Promise<AppSnapshot> {
  return invoke<AppSnapshot>("load_app_snapshot");
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_settings", { settings });
}

/** Opens the system folder dialog and validates the choice. Returns the accepted path. */
export function chooseWorkFolder(): Promise<string> {
  return invoke<string>("choose_work_folder");
}

/** Create `~/AssistantCabinetAI` if it is missing, then return the accepted path. */
export function ensureSuggestedWorkFolder(): Promise<string> {
  return invoke<string>("ensure_suggested_work_folder");
}

export function checkServerHealth(): Promise<HealthSnapshot> {
  return invoke<HealthSnapshot>("check_server_health");
}

/**
 * Send the conversation and receive the answer as it is written. Rust holds the server address,
 * the model alias and the output locale, so the view cannot send a request to somewhere else.
 */
export function sendChatMessage(
  turns: ChatTurn[],
  onDelta: (text: string) => void,
): Promise<string> {
  const channel = new Channel<ChatStreamEvent>();
  channel.onmessage = (message) => {
    if (message.event === "delta") {
      onDelta(message.text);
    }
  };
  return invoke<string>("send_chat_message", { turns, onEvent: channel });
}

/**
 * Stop the question being worked on, wherever it has got to: embedding it, searching the index, or
 * streaming the answer. The command returns at once; the stopped request then rejects with
 * `chat_cancelled`, which is how the interface learns it really ended.
 */
export function cancelChat(): Promise<void> {
  return invoke<void>("cancel_chat");
}

/** One pass of discovery, extraction, chunking and embedding over the work folder. */
export function indexWorkFolder(): Promise<IndexSummary> {
  return invoke<IndexSummary>("index_work_folder");
}

/** Whether the local index has anything to search yet - a work folder can be chosen but never
 * analysed. Used to say so instantly, before spending a round trip on a question retrieval is
 * certain to refuse. */
export function hasIndexedDocuments(): Promise<boolean> {
  return invoke<boolean>("has_indexed_documents");
}

/**
 * Retrieval, then a sourced chat answer. Rejects with `insufficient_evidence` rather than
 * answering when the local index carries nothing relevant.
 */
export function askWithSources(
  question: string,
  onDelta: (text: string) => void,
  onSources: (sources: Evidence[], coverage: EvidenceCoverage | null) => void,
  onFolderAnswer?: (answer: FolderAnswer) => void,
  /** Ask the model even when the work folder could answer on its own. Her choice, made on an
   * answer she has already seen. */
  skipDeterministic = false,
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
