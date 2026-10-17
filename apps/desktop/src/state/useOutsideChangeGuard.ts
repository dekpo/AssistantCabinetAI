import { useEffect, useRef } from "react";
import { dataFolderInventory, workFolderInventory } from "../lib/ipc";
import { outsideChange, type OutsideChange } from "../lib/viewerGuard";
import type { WorkspaceView } from "../lib/workspaceView";

/**
 * A file open in the centre can be renamed, moved, deleted or edited in the file manager while the
 * window is behind. Active only while a file is open: when the window gets the focus back, the
 * inventory of the file's own folder is read again (cheap, thanks to the hash cache) and compared
 * with the record that was opened (`outsideChange`). A difference calls `onChanged`. No watcher and
 * no timer: the check happens at the one moment the user comes back to look.
 */
export function useOutsideChangeGuard(
  workspace: WorkspaceView | null,
  onChanged: (change: OutsideChange) => void,
): void {
  const open = workspace?.kind === "file" ? workspace : null;
  const openRef = useRef(open);
  openRef.current = open;
  const active = open !== null;

  useEffect(() => {
    if (!active) {
      return;
    }
    const onFocus = () => {
      const checked = openRef.current;
      if (checked === null) {
        return;
      }
      const read = checked.folder === "documents" ? workFolderInventory : dataFolderInventory;
      read()
        .then((report) => {
          /* The user may have closed the file or opened another while the inventory was read. */
          if (openRef.current !== checked) {
            return;
          }
          const change = outsideChange(checked.record, report.files);
          if (change !== null) {
            onChanged(change);
          }
        })
        .catch(() => {
          // An inventory that cannot be read decides nothing: the next focus tries again.
        });
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [active, onChanged]);
}
