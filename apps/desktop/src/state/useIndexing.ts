import { useCallback, useRef, useState } from "react";
import { normaliseError, type AppError } from "../lib/errors";
import { indexWorkFolder, type IndexProgress, type IndexSummary } from "../lib/ipc";

/**
 * One analysis pass over the work folder, owned above both places that start it: the folder card
 * in the sidebar, and the answer that had to say "analyse your documents first". Two buttons, one
 * pass - a second pass started while the first is running would re-embed the same files.
 */
export interface IndexingState {
  running: boolean;
  /** How far the running pass has got, or null when nothing is running. */
  progress: IndexProgress | null;
  /** What the last finished pass did. Cleared when a new pass starts. */
  summary: IndexSummary | null;
  error: AppError | null;
  /** How many passes have finished, successfully or not. The folder card re-reads its inventory
   * from this rather than from `summary`, so a failed pass still refreshes what is on screen. */
  finishedPasses: number;
  /** Resolves true when the pass finished without failing, so a caller can go on to do the thing
   * the missing index was blocking. */
  run: () => Promise<boolean>;
  /** Forget the last pass. Used when the folder itself changes: its summary describes a folder
   * that is no longer the one on screen. */
  reset: () => void;
}

export function useIndexing(): IndexingState {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [summary, setSummary] = useState<IndexSummary | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [finishedPasses, setFinishedPasses] = useState(0);
  /* Read rather than `running` so a second click in the same render cannot start a second pass. */
  const inFlight = useRef(false);

  const run = useCallback(async (): Promise<boolean> => {
    if (inFlight.current) {
      return false;
    }
    inFlight.current = true;
    setError(null);
    setSummary(null);
    setProgress(null);
    setRunning(true);
    try {
      setSummary(await indexWorkFolder(setProgress));
      return true;
    } catch (raw: unknown) {
      setError(normaliseError(raw));
      return false;
    } finally {
      inFlight.current = false;
      setRunning(false);
      setProgress(null);
      setFinishedPasses((count) => count + 1);
    }
  }, []);

  const reset = useCallback(() => {
    setSummary(null);
    setError(null);
  }, []);

  return { running, progress, summary, error, finishedPasses, run, reset };
}
