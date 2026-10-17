import type { FileRecord } from "./ipc";

/** Why an open file can no longer be shown as it was opened. */
export type OutsideChange = "missing" | "not_analysed" | "modified";

/**
 * Compares the record that was opened with a fresh inventory of the same folder. The inventory
 * already reports a changed file as not analysed, so the hash comparison is redundant with the
 * status; it costs nothing and is kept. `null` means the file is as it was.
 */
export function outsideChange(open: FileRecord, fresh: readonly FileRecord[]): OutsideChange | null {
  const current = fresh.find((file) => file.relativePath === open.relativePath);
  if (current === undefined) {
    return "missing";
  }
  if (current.processingStatus !== "indexed") {
    return "not_analysed";
  }
  if (current.sha256 !== open.sha256) {
    return "modified";
  }
  return null;
}
