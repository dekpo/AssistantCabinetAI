import type { AnalysisScope, FileRecord } from "./ipc";

/** The default: nothing is narrowed, which is how every question behaved before scopes existed. */
export function wholeFolderScope(now: number): AnalysisScope {
  return { mode: { kind: "whole_folder" }, createdAt: now, updatedAt: now };
}

/** The relative paths an explicit scope keeps; empty for the whole folder. */
export function scopedPaths(scope: AnalysisScope): string[] {
  return scope.mode.kind === "explicit"
    ? scope.mode.entries.map((entry) => entry.relativePath)
    : [];
}

/**
 * The scope after `file` is ticked or unticked.
 *
 * Ticking pins the file's `id` at that moment, so Rust can tell when it was replaced afterwards.
 * Unticking removes the entry, so ticking it again is a new choice with a fresh pin. Unticking the
 * last file returns to the whole folder, never to a scope that allows nothing.
 */
export function toggleScopeFile(
  scope: AnalysisScope,
  file: FileRecord,
  now: number,
): AnalysisScope {
  const entries = scope.mode.kind === "explicit" ? scope.mode.entries : [];
  const present = entries.some((entry) => entry.relativePath === file.relativePath);
  const next = present
    ? entries.filter((entry) => entry.relativePath !== file.relativePath)
    : [...entries, { relativePath: file.relativePath, pinnedId: file.id, addedAt: now }];

  if (next.length === 0) {
    return { ...wholeFolderScope(scope.createdAt), updatedAt: now };
  }
  return { mode: { kind: "explicit", entries: next }, createdAt: scope.createdAt, updatedAt: now };
}
