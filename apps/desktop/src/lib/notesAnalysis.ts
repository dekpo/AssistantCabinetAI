import { countPendingDocuments } from "./analysis";
import type { FileRecord } from "./ipc";

/**
 * Saving a note runs the existing Analyse pass over the whole folder (the per-file path is not
 * separable from it), so before it starts the user must know whether it would also read other
 * documents. This counts those: the documents the pass would still change (same rule as the
 * Documents card's button), minus the notes just saved, which she asked to analyse by saving.
 *
 * The inventory held by the interface may not yet list a note written a moment ago, or may list it
 * as `discovered`; both are handled by leaving the saved paths out. Paths are compared without
 * regard to case, because Windows and macOS disks usually do not tell `Notes` from `notes`.
 */
export function pendingOtherDocuments(files: FileRecord[], savedPaths: string[]): number {
  const saved = new Set(savedPaths.map((path) => path.toLowerCase()));
  return countPendingDocuments(files.filter((file) => !saved.has(file.relativePath.toLowerCase())));
}

/** Whether the save must ask before it analyses: only when other documents are also pending. */
export function needsAnalysisConfirmation(files: FileRecord[], savedPaths: string[]): boolean {
  return pendingOtherDocuments(files, savedPaths) > 0;
}
