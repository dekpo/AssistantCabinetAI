import type { FileRecord, IndexProgress } from "./ipc";

/**
 * Whether the folder still holds a file the index has not read, so the interface can say "do this
 * next" with a filled button and then stop shouting once there is nothing left to do.
 *
 * `failed` does not count: a file that could not be read will not become readable by analysing it
 * again, and a button that stays lit for ever teaches her to ignore it. `discovered` (never read)
 * and `pending` (changed since it was read) do count - those are the two cases one more pass
 * actually resolves.
 */
export function analysisPending(files: FileRecord[]): boolean {
  return files.some(
    (file) => file.processingStatus === "discovered" || file.processingStatus === "pending",
  );
}

/**
 * Where to draw the bar, as a fraction between 0 and 1. A pass over an empty folder is finished
 * the moment it starts, which is the only honest thing a bar can say about no work at all.
 */
export function analysisFraction(progress: IndexProgress): number {
  if (progress.totalFiles <= 0) {
    return 1;
  }
  const fraction = progress.processedFiles / progress.totalFiles;
  return Math.min(1, Math.max(0, fraction));
}
