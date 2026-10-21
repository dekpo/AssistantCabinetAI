import type { FileRecord } from "./ipc";

/**
 * How many files one more pass would still change, so the interface can say "do this next" with a
 * filled button and then stop shouting once there is nothing left to do.
 *
 * `failed` does not count: a file that could not be read will not become readable by analysing it
 * again, and a button that stays lit for ever teaches her to ignore it. `discovered` (never read)
 * and `pending` (changed since it was read) do count - those are the two cases one more pass
 * actually resolves.
 */
export function countPending(files: FileRecord[]): number {
  return files.filter(
    (file) => file.processingStatus === "discovered" || file.processingStatus === "pending",
  ).length;
}

/**
 * The share of the meaning search computed, as a whole percent, while it is incomplete; null when
 * nothing is known, there is nothing to compute, or every passage has its vector (the card says
 * nothing at 100 %).
 */
export function unfinishedPercent(share: { done: number; total: number } | null): number | null {
  if (share === null || share.total <= 0 || share.done >= share.total) {
    return null;
  }
  return Math.floor((100 * share.done) / share.total);
}

/** The kinds the document pass reads; mirrors `FileKind::is_document` in `file_record.rs`. */
const DOCUMENT_KINDS: ReadonlySet<FileRecord["kind"]> = new Set([
  "document_text",
  "document_pdf",
  "document_office",
  "image",
]);

/**
 * `countPending` for the document pass: only files the document pass can read. A `.pages`, a
 * `.key` or a workbook dropped in that folder never gets an index row, so it stays `discovered`
 * for ever and would keep the button lit with a number analysing can never bring down. Same rule
 * as `WorkFolderInventory::unanalysed_documents` on the Rust side, which feeds the answers'
 * "not analysed" warning - the button and an answer cannot disagree. The tabular pass
 * keeps `countPending`: its files are all `tabular_candidate`.
 */
export function countPendingDocuments(files: FileRecord[]): number {
  return countPending(files.filter((file) => DOCUMENT_KINDS.has(file.kind)));
}
