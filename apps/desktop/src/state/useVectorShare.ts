import { useEffect, useState } from "react";
import { vectorShare, type VectorShare } from "../lib/ipc";

/**
 * How much of the meaning search is computed, by one cheap lookup at launch and after each finished
 * pass. The card shows it only while it is below 100 %. Nothing is computed by looking, and a failed
 * lookup says nothing.
 */
export function useVectorShare(folder: string | null, finishedPasses: number): VectorShare | null {
  const [share, setShare] = useState<VectorShare | null>(null);

  useEffect(() => {
    if (folder === null) {
      setShare(null);
      return;
    }
    let current = true;
    vectorShare().then(
      (found) => {
        if (current) {
          setShare(found);
        }
      },
      () => {
        if (current) {
          setShare(null);
        }
      },
    );
    return () => {
      current = false;
    };
  }, [folder, finishedPasses]);

  return share;
}
