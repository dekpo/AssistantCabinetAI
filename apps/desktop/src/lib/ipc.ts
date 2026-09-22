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

export type ChatStreamEvent =
  | { event: "delta"; text: string }
  | { event: "completed"; text: string }
  | { event: "sources"; sources: Evidence[] };

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
  onSources: (sources: Evidence[]) => void,
): Promise<AskAnswer> {
  const channel = new Channel<ChatStreamEvent>();
  channel.onmessage = (message) => {
    if (message.event === "delta") {
      onDelta(message.text);
    } else if (message.event === "sources") {
      // Read from the event rather than from the resolved answer, because a stopped answer never
      // resolves and still has to show where its text came from.
      onSources(message.sources);
    }
  };
  return invoke<AskAnswer>("ask_with_sources", { question, onEvent: channel });
}
