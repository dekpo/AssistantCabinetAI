import type { FillPlan, FillPreview, FillReport, FillRequest } from "./fill";
import { Channel, invoke } from "@tauri-apps/api/core";

/**
 * The only bridge to the outside. The webview never calls the gateway itself: the address, the
 * context cap, the path allow-list and the stored settings all live behind these commands, in
 * Rust, where a change to the view cannot loosen them.
 */

export type ThemeChoice = "light" | "dark" | "system";

/**
 * Whether the local knowledge base reads the analysed files for names. `off` leaves the product as
 * it was before it and writes nothing; `suggest` is the default and never narrows a search by
 * itself; `auto` narrows the search to the files that match the names a question holds.
 */
export type KnowledgeMode = "off" | "suggest" | "auto";

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
  /** Reveals the developer panel under the last answer (`DiagnosticsPanel`). Never shown in normal
   * chat. Off by default. */
  showDiagnostics: boolean;
  /** Appends numbers and machine codes, never a question or a file name, to
   * `retrieval-timings.jsonl` and `analysis-timings.jsonl` in the app-data folder. Off by default. */
  writeTimingLog: boolean;
  knowledgeMode: KnowledgeMode;
  /** The optional lexicon packs the knowledge base reads names with, named `<domain>-<country>`:
   * `health-fr` (France), `health-ch` (Switzerland: AVS / AHV number, Swiss IBAN and telephone),
   * `legal-fr`, `accounting-fr`, and later `health-eu`, `legal-ch`... Rust keeps only the ids a
   * build ships and reads an older bare id (`health`) as its French pack. */
  knowledgePacks: string[];
  /** Below this many selected files on one side (documents, or workbooks) the search is not
   * narrowed by itself. A name the user picked as a chip narrows at any size. Rust clamps it. */
  knowledgeMinScopeFiles: number;
  /** A name present in more than this share of the selected files names no file in particular and
   * is ignored. 0.05 to 1. */
  knowledgeMaxSelectivity: number;
  /** A narrowing applies only when it leaves out at least this share of the selection. 0 to 0.95. */
  knowledgeMinReduction: number;
  /** At most this many documents are searched after a narrowing; workbooks are not capped. */
  knowledgeMaxCandidates: number;
  /** When the closest passage of a narrowed search is below this similarity, the search runs again
   * on the whole selection. The scale belongs to the embedding model (0.65 on `nomic-embed-text`). */
  knowledgeWidenBelowScore: number;
  /** Question suggestions leave names out, because a screen is visible to whoever sits across
   * from the user. On by default. */
  knowledgeHideNamesInSuggestions: boolean;
  /** The copy buttons (one answer, or the whole conversation) also copy the sources listed under
   * each answer. Off by default. */
  copySources: boolean;
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
  | { kind: "nothing_selected" }
  /** "Which documents talk about X": a word search of the selected files, answered from the index
   * with no model. The first files carry their excerpt, the others only their path, and the total
   * is always stated. */
  | {
      kind: "document_search";
      query: string;
      /** The folded words that were read: the answer says it is a word search. */
      terms: string[];
      prefix: boolean;
      quoted: boolean;
      /** The business type the collection noun stood for (`invoice`), applied as a filter. */
      businessType: string | null;
      /** X was an identifier, searched in every spelling. */
      identifier: boolean;
      total: number;
      listed: DocumentListing[];
      others: string[];
      coverage: FinderCoverage;
    }
  /** "What is the latest letter received": the readable documents ranked by date. The first of
   * `ranking` is the answer. */
  | {
      kind: "ranked_file";
      latest: boolean;
      /** The collection noun as written, null when it names no kind of document. */
      noun: string | null;
      /** The readable documents that were ranked or could not be. */
      candidates: number;
      /** Of those, the ones with no date at all. */
      unranked: number;
      /** The dates are not all of one kind. */
      mixedBasis: boolean;
      ranking: RankedEntry[];
    };

/** One file of a ranking: its date, and where the date comes from. */
export interface RankedEntry {
  relativePath: string;
  /** `YYYY-MM-DD`. */
  date: string;
  /** `filename` (the date at the start of the name) or `modified` (the modification date). */
  basis: "filename" | "modified";
}

/** One file of a set answer, with the passage that matched. */
export interface DocumentListing {
  relativePath: string;
  title: string | null;
  businessType: string | null;
  excerpt: string | null;
  page: number | null;
}

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
  /** What the knowledge base did to the search, sent just before the sources. Only when there is
   * something to tell. */
  | { event: "targeting"; report: TargetingReport }
  | { event: "sources"; sources: Evidence[]; coverage: EvidenceCoverage | null }
  | { event: "folder_answer"; answer: FolderAnswer };

/**
 * What the client asks of the knowledge base for one question (`TargetingRequest` in Rust).
 * `mode: "off"` is the widen action: the old search on the whole selection. `entityIds` are chips
 * the user picked; `carryOver` is the previous answer's entities, honoured only for a short
 * follow-up and held here, never stored by Rust.
 */
export interface TargetingRequest {
  mode?: "auto" | "off";
  entityIds?: number[];
  carryOver?: number[];
}

/** An entity of the selection, as the interface may show it. The display is the entity's own
 * name: user data, shown on screen only. */
export interface TargetingEntity {
  entityId: number;
  display: string;
  typeId: string;
  confidence: number;
  /** `chip`, `carried_over`, `canonical_name`, `alias`, `identifier`. */
  method: string;
}

export interface TargetingCandidate {
  entityId: number;
  display: string;
  typeId: string;
  /** How many files of the current selection name it, by side. A candidate that names none is not
   * in the selection and is never offered (invariant I7, checked again in `lib/targeting.ts`). */
  documents: number;
  workbooks: number;
}

/** What the knowledge base did to the search (`TargetingReport` in Rust). Machine codes and ids;
 * the interface writes the sentence. */
export interface TargetingReport {
  /** `entity_files`, `chip_files` or `none`. */
  strategy: string;
  /** Why nothing was reduced, when nothing was (`off`, `named_file`, `small_scope`, `no_entity`,
   * `no_discriminating_entity`, `ambiguous`, `no_candidates`, `insignificant`, `lookup_error`,
   * `user_widened`). */
  reason: string | null;
  requestedBy: "auto" | "chip";
  /** `auto` when the reduced search found too little and ran again on the whole selection. */
  widened: "none" | "auto";
  selectedDocuments: number;
  candidateDocuments: number;
  selectedWorkbooks: number;
  candidateWorkbooks: number;
  reductionPercent: number;
  entities: TargetingEntity[];
  candidatePaths: string[];
  workbookPaths: string[];
  /** A name that fits several entities inside the selection: asked, never picked. `span` is the
   * words of the question that fit them, exactly as the user typed them. */
  ambiguities: { span: string; candidates: TargetingCandidate[] }[];
  /** Probably one of these (`phonetic`, `token_set`, `initial`, `shared_identifier`). */
  possibles: { span: string; candidates: TargetingCandidate[]; reason: string; score: number }[];
  chosenWorkbook: { file: string; reason: string } | null;
}

/** One entity the `@` picker offers: a name of the current selection, with how many of its files
 * name it. */
export interface EntityChoice {
  entityId: number;
  display: string;
  typeId: string;
  documents: number;
  workbooks: number;
}

/** A name that exists only in analysed files the user did not select: counts and paths, never a
 * sentence. The one surface allowed to mention it, and always labelled as outside the selection
 * (invariant I7). */
export interface OutsideSelection {
  entityId: number;
  display: string;
  typeId: string;
  documents: string[];
  workbooks: string[];
}

export interface EntitySearch {
  entities: EntityChoice[];
  outsideSelection: OutsideSelection[];
}

/** The names of the current selection that begin with `prefix` (`knowledge_search_entities`). An
 * empty selection finds nothing, whatever the base holds. */
export function searchEntities(
  prefix: string,
  scope: CombinedScope,
  limit?: number,
): Promise<EntitySearch> {
  return invoke<EntitySearch>("knowledge_search_entities", { prefix, scope, limit });
}

/** The kinds of question the knowledge base can suggest. The sentence of each is a catalogue entry
 * (`suggestions.<id>`); Rust sends the id and the slots, never a sentence. */
export type SuggestionTemplate =
  | "entity_documents"
  | "entity_rows_count"
  | "entity_total"
  | "entity_period_count"
  | "entity_compare_docs_data"
  | "top_group"
  | "each_document"
  | "folder_count";

/** One question worth asking, as data. Every one has passed a dry run on the user's own files. */
export interface Suggestion {
  templateId: SuggestionTemplate;
  entity: { entityId: number; display: string } | null;
  /** The text of the table cell the name was read from, as the table writes it. */
  value: string | null;
  column: string | null;
  group: string | null;
  /** The workbook the question is about, set when the selection holds several. */
  file: string | null;
  month: number | null;
  year: number | null;
}

export interface SuggestionSet {
  suggestions: Suggestion[];
  /** Names were left out because the setting hides them and the selection holds some. */
  namesHidden: boolean;
  dryRuns: number;
}

/** The questions worth asking about the current selection (`knowledge_suggest_questions`). Names are
 * left out unless the setting allows them or `revealNames` (the user's own click, kept for the
 * session) asks for them. A read: no model, no write. */
export function suggestQuestions(
  scope: CombinedScope,
  revealNames: boolean,
): Promise<SuggestionSet> {
  return invoke<SuggestionSet>("knowledge_suggest_questions", { scope, revealNames });
}

/** What the knowledge base knows about one name inside the current selection
 * (`knowledge_entity_card`). Files outside the selection are counted, never named. */
export interface EntityCard {
  entityId: number;
  display: string;
  typeId: string;
  subtype: string | null;
  status: "active" | "candidate";
  aliases: string[];
  roles: { role: string; mentions: number }[];
  documents: { relativePath: string; mentions: number; pages: number[] }[];
  tables: { relativePath: string; sheet: string; column: string; rows: number }[];
  outside: { documents: number; workbooks: number };
  relations: {
    predicate: string;
    otherEntityId: number;
    otherDisplay: string;
    direction: "subject" | "object";
  }[];
  attributes: { key: string; value: string }[];
}

export function entityCard(entityId: number, scope: CombinedScope): Promise<EntityCard | null> {
  return invoke<EntityCard | null>("knowledge_entity_card", { entityId, scope });
}

/** The recorded questions added up (`knowledge_diagnostics`). Counts and ratios only. */
export interface KnowledgeAggregates {
  questions: number;
  withEntities: number;
  withEntitiesShare: number | null;
  wouldApply: number;
  wouldApplyRate: number | null;
  applied: number;
  widened: number;
  medianReductionPercent: number | null;
  recallOverall: number | null;
  recallWhenWouldApply: number | null;
  missNoMention: number;
  missCapped: number;
  missEntityNotResolved: number;
  skippedByReason: Record<string, number>;
}

export function knowledgeDiagnostics(limit?: number): Promise<KnowledgeAggregates> {
  return invoke<KnowledgeAggregates>("knowledge_diagnostics", { limit });
}

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
  /** The pass has read every file and is now reading the names in documents analysed earlier:
   * `processedFiles` and `totalFiles` then count those documents. */
  readingNames?: boolean;
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

/**
 * What a pass learned about names, as counts. Never a name: the interface writes the sentence
 * (`analysis.knowledge.*`) and Rust builds none.
 */
export interface KnowledgeSummary {
  /** People and organisations named in the files this pass read. */
  entitiesDetected: number;
  /** Of those, how many the knowledge base already held. */
  matchedExisting: number;
  createdNew: number;
  /** Of the new ones, how many are only guesses until a second file names them. */
  candidates: number;
  /** E-mail addresses, invoice numbers, IBANs and the like. */
  identifiers: number;
  /** Links a table states by two role columns. */
  relations: number;
  /** Columns of the Data Folder's tables read as a name, an organisation, an identifier... */
  semanticColumns: number;
  /** Workbooks whose columns and values were read by this pass (not the unchanged ones). */
  workbooksAnalysed: number;
  /** Documents analysed earlier that were read again from their stored text. */
  refreshedSources: number;
  truncatedSources: number;
  /** Files indexed whose names could not all be recorded: analysing again retries them. */
  errors: number;
  elapsedMs: number;
}

export interface UnreadFormat {
  extension: string;
  count: number;
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
  /** Files no reader of the product opens, counted by extension (no dot), most numerous first. */
  unreadFormats: UnreadFormat[];
  chunkCount: number;
  /** Where the pass spent its time. Numbers only. */
  timings: AnalysisTimings | null;
  /** Null when the knowledge base took no part (`knowledgeMode: "off"`). */
  knowledge?: KnowledgeSummary | null;
  /** What the file profile pass did, as counts. Null when it could not start; it never blocks a pass. */
  profile?: ProfileSummary | null;
}

/** The file profile pass of an Analyse (`file_profile::store`). Counts only, never a name. */
export interface ProfileSummary {
  built: number;
  relinked: number;
  unchanged: number;
  removed: number;
  errors: number;
  /** False when the pass was cancelled before its last file. */
  complete: boolean;
  /** What the pass cost, in milliseconds. */
  elapsedMs: number;
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
  | { kind: "which_workbook"; candidates: string[]; hints?: WorkbookHint[] }
  | { kind: "ambiguous_reference"; query: string; candidates: string[] }
  | { kind: "file_not_selected"; query: string }
  | { kind: "no_matching_file"; query: string }
  | { kind: "no_usable_table" };

/** Why the knowledge base points at a workbook of "which workbook?": the entities of the question
 * it holds and the sheets they appear in. Orders and annotates the choice, never makes it. */
export interface WorkbookHint {
  file: string;
  entities: { entityId: number; display: string }[];
  sheets: string[];
}

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
  /** What the knowledge base did to the search, when there is something to tell: an applied
   * reduction, or names that need a question. */
  targeting: TargetingReport | null;
}

export type RetrievalPath =
  | "documents"
  | "tabular"
  | "mixed"
  | "folder_answer"
  | "document_search"
  | "without_documents";

export type RetrievalPlan = "one_file" | "every_document" | "whole_folder";

/**
 * The timings and counts of one question (`knowledge::diagnostics` in Rust). Every `...Ms` is the
 * wall time of one stage, and stays 0 when the question did not go through it. Shown only by the
 * developer panel, when `AppSettings.showDiagnostics` is on.
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
  /** The knowledge base's part: lookup time, what it found, what it reduced, how a reduction that
   * was only measured compares with what the answer cited. Absent when the base took no part. */
  targeting: TargetingDiagnostics | null;
}

/** One entity of the question, without a letter of its name. */
export interface QueryEntityDiagnostic {
  entityId: number;
  typeId: string;
  confidence: number;
  method: string;
}

export interface TargetingDiagnostics {
  mode: "off" | "suggest" | "auto";
  knowledgeLookupMs: number;
  strategy: string;
  skippedReason: string | null;
  outcome: "applied" | "shadow" | "skipped";
  queryEntities: QueryEntityDiagnostic[];
  candidateDocuments: number;
  candidateWorkbooks: number;
  reductionPercent: number;
  widened: boolean;
  shadow: {
    wouldApply: boolean;
    cited: number;
    citedInCandidates: number;
    missNoMention: number;
    missCapped: number;
    missEntityNotResolved: number;
  };
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

/**
 * Whether each folder's analysis has its file profile (`profile_index_status`): `needsAnalyse` means
 * the folder holds analysed files and no profile of the current layout, so the interface says the
 * index must be redone. One cheap lookup at launch; only an Analyse the user starts fills it.
 */
export type FolderProfileStatus = "empty" | "current" | "needsAnalyse";

export interface ProfileIndexStatus {
  documents: FolderProfileStatus;
  data: FolderProfileStatus;
}

export function profileIndexStatus(): Promise<ProfileIndexStatus> {
  return invoke<ProfileIndexStatus>("profile_index_status");
}

/** Which of the two folders a finder call searches. */
export type FinderDomain = "documents" | "data";

/** What the finder knows about a file's state: the profile's own, or why there is none. */
export type FinderFileState =
  | "analysed"
  | "empty"
  | "no_text"
  | "cells_missing"
  | "unanalysed"
  | "unsupported";

export interface FinderEntityFilter {
  entityId: number;
  /** With a role only the profile is read; without one the knowledge base counts too. */
  role?: string | null;
}

/** An end is `YYYY`, `YYYY-MM` or `YYYY-MM-DD`; a month or a year covers all of it. */
export interface FinderDateRange {
  from?: string | null;
  to?: string | null;
}

export interface FinderQuery {
  /** Words typed by the user: matched as words, the last one may be a start. Never meaning. */
  text?: string | null;
  entities?: FinderEntityFilter[];
  /** Business type codes. */
  kinds?: string[];
  date?: FinderDateRange | null;
  /** Folders relative to the work folder: every file under one of them. */
  folders?: string[];
  /** Extensions, lower case, without the dot. */
  formats?: string[];
  states?: FinderFileState[];
  limit?: number | null;
  offset?: number;
  /** The files and their state are wanted, not their cards or excerpts: the left panel selects the
   * files found. */
  pathsOnly?: boolean;
}

/** The finder never changes the selection: a result is a list, and selecting is the user's action. */
export type FinderScope = { kind: "wholeFolder" } | { kind: "selection"; paths: string[] };

/**
 * Why a file was found. `word` means a typed word met a text of the file and `detail` says which:
 * `name`, `title`, `author`, `subject`, `entities`, `headers`, `cells`, `identifiers`, `profile`
 * (the words are spread over several texts) or `passage`. `entity` means a filter linked it, and
 * `detail` is the role or `mention`.
 */
export interface FinderReason {
  kind: "word" | "entity";
  detail: string | null;
}

/** `entityId` is null for a surface the knowledge base does not know: shown as possible, never a filter. */
export interface FinderCardEntity {
  entityId: number | null;
  surface: string;
  role: string;
  confidence: number;
}

export interface FinderCard {
  title: string | null;
  businessType: string | null;
  documentDate: string | null;
  format: string;
  state: FinderFileState;
  entities: FinderCardEntity[];
}

export interface FinderHit {
  domain: FinderDomain;
  relativePath: string;
  card: FinderCard;
  reasons: FinderReason[];
  /** The passage that matched, for the files of the page only. */
  excerpt: string | null;
  locator: { page: number | null; section: number | null } | null;
}

/** `label` is the entity's canonical name in the entity facet, null elsewhere. */
export interface FinderFacet {
  value: string;
  count: number;
  label: string | null;
}

/** Counted over the whole result, not over the page. */
export interface FinderFacets {
  byType: FinderFacet[];
  byYear: FinderFacet[];
  byEntity: FinderFacet[];
  byFolder: FinderFacet[];
  byFormat: FinderFacet[];
  byState: FinderFacet[];
}

/**
 * What the search could not look into, over the files of the scope before the filters:
 * `searched` were analysed, `unreadable` hold no text or are a format the product does not read,
 * `unanalysed` are findable by name only, `partial` is the part of `searched` whose words were cut.
 */
export interface FinderCoverage {
  searched: number;
  unreadable: number;
  unanalysed: number;
  partial: number;
}

export interface FinderResult {
  domain: FinderDomain;
  /** Every file that answers, whatever the page. */
  total: number;
  offset: number;
  limit: number;
  files: FinderHit[];
  facets: FinderFacets;
  coverage: FinderCoverage;
  /** Present when words were typed: the interface says it is a word search. */
  textSearch: { terms: string[]; lastIsPrefix: boolean } | null;
}

/**
 * Find files of one folder with no model, no embedding and no file read; cheap enough to run
 * while the user types.
 */
export function findFiles(
  domain: FinderDomain,
  query: FinderQuery,
  scope: FinderScope = { kind: "wholeFolder" },
): Promise<FinderResult> {
  return invoke<FinderResult>("find_files", { domain, query, scope });
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
  /** What the knowledge base is asked to do: chips, the previous answer's entities, or `off` to
   * widen. Left out, the knowledge mode of the settings decides. */
  targeting?: TargetingRequest,
  /** What the knowledge base did to the search, sent just before the sources so an answer that is
   * stopped still shows it. */
  onTargeting?: (report: TargetingReport) => void,
): Promise<AskAnswer> {
  const channel = new Channel<ChatStreamEvent>();
  channel.onmessage = (message) => {
    if (message.event === "delta") {
      onDelta(message.text);
    } else if (message.event === "targeting") {
      onTargeting?.(message.report);
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
    targeting,
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

// ---------------------------------------------------------------------------------------------
// The knowledge management dialog (lot 8)
//
// Machine data only: ids, counts, codes, locators. Every sentence the dialog shows is the
// catalogue's. Nothing here reads or writes a file of either folder.
// ---------------------------------------------------------------------------------------------

/** What every write reports: the ids it produced, and how many documents the next Analyse reads
 * again because of it (from the text already stored, with no request to the AI). */
export interface ManageOutcome {
  entityId: number | null;
  aliasId: number | null;
  /** For a new name: how many other entities already answer to it. */
  sharedWith: number;
  documentsDue: number;
}

export type EntityListView = "live" | "deleted" | "merged";

export interface EntityListQuery {
  text: string;
  typeId: string | null;
  origin: "automatic" | "manual" | null;
  view: EntityListView;
  /** Only what deserves a look: unconfirmed names, names with a pair to decide on, names seen once. */
  problemsOnly: boolean;
  page: number;
}

export interface EntityRow {
  entityId: number;
  typeId: string;
  subtype: string | null;
  name: string;
  disambiguator: string;
  status: "candidate" | "active" | "merged" | "deleted";
  origin: "automatic" | "manual";
  mergedInto: number | null;
  mentions: number;
  sources: number;
  aliases: number;
  openMatches: number;
}

export interface EntityPage {
  total: number;
  page: number;
  pageSize: number;
  rows: EntityRow[];
}

export interface EntityBriefView {
  entityId: number;
  typeId: string;
  name: string;
  status: string;
}

export interface AliasView {
  aliasId: number;
  entityId: number;
  display: string;
  kind: string;
  origin: "automatic" | "manual";
  confidence: number;
}

export interface LocatorView {
  kind: "chunk" | "cells" | "filename" | "metadata";
  page: number | null;
  section: number | null;
  sheet: string | null;
  column: string | null;
  firstRow: number | null;
  sampleRows: number[];
}

export interface Excerpt {
  text: string;
  cutBefore: boolean;
  cutAfter: boolean;
}

export interface MentionView {
  mentionId: number;
  entityId: number;
  locator: LocatorView;
  excerpt: Excerpt | null;
  alias: string | null;
  role: string | null;
  method: string;
  confidence: number;
  origin: "automatic" | "manual";
  occurrences: number;
  splitFrom: number | null;
}

export interface SourceMentions {
  sourceId: number;
  domain: "documents" | "data";
  relativePath: string;
  mentions: MentionView[];
}

export interface RelationView {
  relationId: number;
  predicate: string;
  subject: EntityBriefView;
  object: EntityBriefView;
  relativePath: string | null;
  occurrences: number;
}

export interface AttributeView {
  key: string;
  value: string;
  origin: "automatic" | "manual";
}

export interface PossibleView {
  other: EntityBriefView;
  reason: string;
  score: number;
  state: "open" | "dismissed" | "merged";
}

export interface ColumnView {
  sheet: string;
  column: string;
  semanticType: string;
  subtype: string | null;
  role: string | null;
  confidence: number;
  reason: string;
  read: boolean;
}

export interface WorkbookColumns {
  relativePath: string;
  columns: ColumnView[];
}

export interface EntityDetail {
  entity: EntityRow;
  effectiveId: number;
  /** The survivor, when this entity was merged into another. */
  survivor: EntityBriefView | null;
  aliases: AliasView[];
  mergedIn: EntityBriefView[];
  sources: SourceMentions[];
  mentionCount: number;
  relations: RelationView[];
  attributes: AttributeView[];
  possibleMatches: PossibleView[];
  columns: WorkbookColumns[];
}

export interface PossiblePair {
  one: EntityBriefView;
  other: EntityBriefView;
  oneMentions: number;
  otherMentions: number;
  reason: string;
  score: number;
}

export interface PossiblePage {
  total: number;
  page: number;
  pageSize: number;
  pairs: PossiblePair[];
}

export interface SourceBrief {
  sourceId: number;
  domain: "documents" | "data";
  relativePath: string;
}

export interface Counted<T> {
  total: number;
  items: T[];
}

export interface HealthReport {
  seenOnce: Counted<EntityBriefView>;
  candidates: number;
  documentsWithoutEntity: Counted<SourceBrief>;
  workbooksWithoutUsableColumn: Counted<SourceBrief>;
  probableDuplicates: number;
  inDocumentsNotInTables: Counted<EntityBriefView>;
}

export interface LastOperation {
  opId: number;
  kind: string;
  undoable: boolean;
  at: number;
}

export interface KnowledgeOverview {
  entitiesLive: number;
  entitiesCandidate: number;
  entitiesManual: number;
  entitiesMerged: number;
  entitiesDeleted: number;
  aliases: number;
  mentions: number;
  documentsRead: number;
  workbooksRead: number;
  openPossibleMatches: number;
  documentsDue: number;
  consistent: boolean;
  lastOperation: LastOperation | null;
}

export interface TypeRow {
  id: string;
  builtin: boolean;
  entities: number;
}

export interface RoleRow {
  id: string;
  labelKey: string;
}

export interface TypesAndRoles {
  types: TypeRow[];
  roles: RoleRow[];
  packs: { id: string; locale: string }[];
  availablePacks: string[];
}

export interface NewManualEntity {
  name: string;
  typeId: string;
  subtype?: string | null;
  disambiguator?: string | null;
}

export interface EditEntity {
  name?: string | null;
  typeId?: string | null;
  disambiguator?: string | null;
}

export interface SplitRequest {
  entityId: number;
  mentionIds: number[];
  sourceIds: number[];
  name: string;
}

export function knowledgeOverview(): Promise<KnowledgeOverview> {
  return invoke<KnowledgeOverview>("knowledge_overview");
}

export function listEntities(query: EntityListQuery): Promise<EntityPage> {
  return invoke<EntityPage>("knowledge_list_entities", { query });
}

export function entitySheet(entityId: number): Promise<EntityDetail> {
  return invoke<EntityDetail>("knowledge_entity_sheet", { entityId });
}

export function possibleMatches(page = 0): Promise<PossiblePage> {
  return invoke<PossiblePage>("knowledge_possible_matches", { page });
}

export function knowledgeHealth(): Promise<HealthReport> {
  return invoke<HealthReport>("knowledge_health");
}

export function knowledgeTypesAndRoles(): Promise<TypesAndRoles> {
  return invoke<TypesAndRoles>("knowledge_types_and_roles");
}

export function createEntity(input: NewManualEntity): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_create_entity", { input });
}

export function editEntity(entityId: number, edit: EditEntity): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_edit_entity", { entityId, edit });
}

export function addAlias(entityId: number, text: string): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_add_alias", { entityId, text });
}

export function removeAlias(aliasId: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_remove_alias", { aliasId });
}

export function setAttribute(entityId: number, key: string, value: string): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_set_attribute", { entityId, key, value });
}

export function removeAttribute(
  entityId: number,
  key: string,
  value: string,
): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_remove_attribute", { entityId, key, value });
}

export function mergeEntities(survivor: number, absorbed: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_merge", { survivor, absorbed });
}

export function unmergeEntity(entityId: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_unmerge", { entityId });
}

export function splitEntity(request: SplitRequest): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_split", { request });
}

export function deleteEntity(entityId: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_delete_entity", { entityId });
}

export function restoreEntity(entityId: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_restore_entity", { entityId });
}

export function acceptMatch(survivor: number, absorbed: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_accept_match", { survivor, absorbed });
}

export function dismissMatch(one: number, other: number): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_dismiss_match", { one, other });
}

export function undoLastOperation(): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_undo");
}

/** "Reset the whole knowledge base": without `includeManual`, what the files taught goes and what
 * the user wrote stays; with it, everything goes. Files, chunks and inventories are never touched. */
export function resetKnowledge(includeManual: boolean): Promise<ManageOutcome> {
  return invoke<ManageOutcome>("knowledge_reset", { includeManual });
}
