import { useCallback, useEffect, useState } from "react";

/**
 * What is actually in one folder, read from the disk and the local index (`docs/WORK-FOLDER-INVENTORY.md`),
 * the way the explorer and the Settings panel read it: once when the folder is chosen, again when the window gets the
 * focus back (a file added in the file manager is the case this catches, without watching the disk),
 * after every finished pass, and when the software itself wrote a file (`refreshToken`).
 *
 * A failed read leaves the box without counts rather than with wrong ones. `read` must be a stable
 * function (a module-level command), not one made on each render.
 */
export function useFolderInventory<Report>(
  read: () => Promise<Report>,
  folder: string | null,
  finishedPasses: number,
  refreshToken = 0,
): Report | null {
  const [inventory, setInventory] = useState<Report | null>(null);

  const refresh = useCallback(() => {
    if (folder === null) {
      setInventory(null);
      return;
    }
    read().then(setInventory, () => setInventory(null));
  }, [read, folder]);

  useEffect(refresh, [refresh]);

  /* `focus` rather than `visibilitychange`: alt-tabbing back to a window that was never hidden fires
     only the first. Rust re-reads a file's bytes only when its size or time moved, so doing this
     often is cheap. */
  useEffect(() => {
    if (folder === null) {
      return;
    }
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [folder, refresh]);

  /* Keyed on the count, not on the summary, so a pass that failed halfway still re-reads what it
     did manage to do. */
  useEffect(() => {
    if (finishedPasses > 0) {
      refresh();
    }
  }, [finishedPasses, refresh]);

  useEffect(() => {
    if (refreshToken > 0) {
      refresh();
    }
  }, [refreshToken, refresh]);

  return inventory;
}
