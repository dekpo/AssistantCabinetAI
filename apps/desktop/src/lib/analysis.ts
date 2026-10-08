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
  return countPending(files) > 0;
}

/**
 * How many files one more pass would still change, by the same rule `analysisPending` uses - so
 * the button cannot be lit while the number beside it reads zero.
 */
export function countPending(files: FileRecord[]): number {
  return files.filter(
    (file) => file.processingStatus === "discovered" || file.processingStatus === "pending",
  ).length;
}

/**
 * Where to draw the bar, as a fraction between 0 and 1. A pass over an empty folder is finished
 * the moment it starts, which is the only honest thing a bar can say about no work at all.
 *
 * Inside a long document the bar moves by its batches, so one 42-page PDF does not look frozen
 * for the minutes it takes. Never more than the file's own share: a file is not done until it is
 * counted in `processedFiles`.
 */
export function analysisFraction(progress: IndexProgress): number {
  if (progress.totalFiles <= 0) {
    return 1;
  }
  const insideFile =
    progress.batchTotal > 0 ? Math.min(1, progress.batchIndex / progress.batchTotal) : 0;
  const fraction = (progress.processedFiles + insideFile) / progress.totalFiles;
  return Math.min(1, Math.max(0, fraction));
}
