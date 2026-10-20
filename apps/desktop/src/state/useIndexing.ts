import { useCallback, useRef, useState } from "react";
import { initialTrace, stepTrace, type ProgressTrace } from "../lib/analysisProgress";
import { normaliseError, type AppError } from "../lib/errors";
import { indexWorkFolder, type IndexProgress, type IndexSummary } from "../lib/ipc";

/**
 * One analysis pass over the work folder, owned above both places that start it: the folder card
 * in the sidebar, and the answer that had to say "analyse your documents first". Two buttons, one
 * pass - a second pass started while the first is running would re-embed the same files.
 *
 * The Data Folder has its own, built from the same hook with `indexDataFolder`: two folders, two
 * independent passes, each shared by the sidebar card and the settings panel.
 */
export interface IndexingState {
  running: boolean;
  /** How far the running pass has got, or null when nothing is running. */
  progress: IndexProgress | null;
  /** What the running pass has reported, with the moment it started, for the gauge's stage, rate and
   * clock (`lib/analysisProgress.ts`). Null when nothing is running. */
  trace: ProgressTrace | null;
  /** How long the last finished pass took, from the press to its end. Cleared when a new pass starts. */
  durationMs: number | null;
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

export function useIndexing(
  pass: (onProgress: (progress: IndexProgress) => void) => Promise<IndexSummary> = indexWorkFolder,
): IndexingState {
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [trace, setTrace] = useState<ProgressTrace | null>(null);
  const [durationMs, setDurationMs] = useState<number | null>(null);
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
    setDurationMs(null);
    const startedAt = Date.now();
    setTrace(initialTrace(startedAt));
    setRunning(true);
    try {
      setSummary(
        await pass((next) => {
          const at = Date.now();
          setProgress(next);
          setTrace((current) =>
            current === null ? current : stepTrace(current, { at, progress: next }),
          );
        }),
      );
      return true;
    } catch (raw: unknown) {
      setError(normaliseError(raw));
      return false;
    } finally {
      inFlight.current = false;
      setDurationMs(Date.now() - startedAt);
      setRunning(false);
      setProgress(null);
      setTrace(null);
      setFinishedPasses((count) => count + 1);
    }
  }, [pass]);

  const reset = useCallback(() => {
    setSummary(null);
    setError(null);
    setDurationMs(null);
  }, []);

  return { running, progress, trace, durationMs, summary, error, finishedPasses, run, reset };
}
