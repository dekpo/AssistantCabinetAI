import { useEffect, useRef } from "react";
import { dataFolderInventory, workFolderInventory, type FileRecord } from "../lib/ipc";
import { tabsGone, type FreshInventories } from "../lib/viewerGuard";
import type { OpenFile, ViewerFolder } from "../lib/workspaceView";

/**
 * Files open in the centre can be renamed, moved, deleted or edited in the file manager while the
 * window is behind. Active only while a tab is open: when the window gets the focus back, the
 * inventory of each folder that has an open tab is read again (at most two reads, cheap thanks to
 * the hash cache) and every tab is compared with the record it was opened with (`tabsGone`). The
 * tabs that differ are reported together in one call, the others are left alone. No watcher and no
 * timer: the check happens at the one moment the user comes back to look.
 */
export function useOutsideChangeGuard(
  tabs: readonly OpenFile[],
  onGone: (ids: string[]) => void,
): void {
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  const active = tabs.length > 0;

  useEffect(() => {
    if (!active) {
      return;
    }
    const onFocus = () => {
      const checked = tabsRef.current;
      const folders = [...new Set(checked.map((tab) => tab.folder))];
      const read = (folder: ViewerFolder): Promise<readonly FileRecord[] | null> =>
        (folder === "documents" ? workFolderInventory : dataFolderInventory)()
          .then((report) => report.files)
          // An inventory that cannot be read decides nothing: the next focus tries again.
          .catch(() => null);
      Promise.all(folders.map(read)).then((inventories) => {
        const fresh: FreshInventories = {};
        folders.forEach((folder, index) => {
          const files = inventories[index];
          if (files !== null) {
            fresh[folder] = files;
          }
        });
        /* The user may have closed a tab, or opened the file again after an Analyse, while the
           inventories were read: only a tab that is still open with the very record that was
           compared can be reported. */
        const now = tabsRef.current;
        const ids = tabsGone(checked, fresh)
          .map((entry) => entry.id)
          .filter((id) => {
            const before = checked.find((tab) => tab.id === id);
            const after = now.find((tab) => tab.id === id);
            return after !== undefined && after.record === before?.record;
          });
        if (ids.length > 0) {
          onGone(ids);
        }
      });
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [active, onGone]);
}
