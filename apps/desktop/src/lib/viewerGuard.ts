import type { FileRecord } from "./ipc";
import type { OpenFile, ViewerFolder } from "./workspaceView";

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

/** A fresh inventory per folder; a folder whose inventory could not be read is absent. */
export type FreshInventories = Partial<Record<ViewerFolder, readonly FileRecord[]>>;

export interface TabGone {
  id: string;
  reason: OutsideChange;
}

/**
 * The open tabs that can no longer be shown as they were opened, each with its reason. A tab is
 * compared only with the inventory of its own folder (the same path can exist in both), and a tab
 * whose folder has no inventory is left alone: an unreadable folder decides nothing.
 */
export function tabsGone(tabs: readonly OpenFile[], fresh: FreshInventories): TabGone[] {
  const gone: TabGone[] = [];
  for (const tab of tabs) {
    const inventory = fresh[tab.folder];
    if (inventory === undefined) {
      continue;
    }
    const reason = outsideChange(tab.record, inventory);
    if (reason !== null) {
      gone.push({ id: tab.id, reason });
    }
  }
  return gone;
}
