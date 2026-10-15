import { useEffect, useState } from "react";
import { vectorShareByFile } from "../lib/ipc";

/**
 * The files whose passages are not all searchable by meaning yet, by one cheap grouped lookup at launch and
 * after each finished pass. The listing marks exactly these files. Nothing is computed by looking, and a
 * failed lookup marks nothing.
 */
export function useFileVectorShares(
  folder: string | null,
  finishedPasses: number,
): Map<string, { chunks: number; vectors: number }> {
  const [shares, setShares] = useState<Map<string, { chunks: number; vectors: number }>>(new Map());

  useEffect(() => {
    if (folder === null) {
      setShares(new Map());
      return;
    }
    let current = true;
    vectorShareByFile().then(
      (found) => {
        if (current) {
          setShares(new Map(found.map((one) => [one.relativePath, one])));
        }
      },
      () => {
        if (current) {
          setShares(new Map());
        }
      },
    );
    return () => {
      current = false;
    };
  }, [folder, finishedPasses]);

  return shares;
}
