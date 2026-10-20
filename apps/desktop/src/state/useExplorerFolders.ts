import { useState } from "react";
import { normaliseError, type AppError } from "../lib/errors";
import type { Family } from "../lib/explorerModel";
import { attemptFolder } from "../lib/explorerStates";
import {
  chooseDataFolder,
  chooseWorkFolder,
  ensureSuggestedDataFolder,
  ensureSuggestedWorkFolder,
  resetDataIndex,
  resetIndex,
  revealDataFolder,
  revealWorkFolder,
} from "../lib/ipc";
import type { IndexingState } from "./useIndexing";

/** Closing the system dialog without choosing is not a failure, so it is not reported as one. Rust's
 * two choose commands return the same code. */
const CANCELLED = "work_folder_selection_cancelled";

/**
 * What the explorer does to a family's folder: choose it (the system dialog, or the suggested folder
 * made on the spot), show it in the file manager, and clear its analysis. The same commands the two
 * old folder cards call, with the same guard: a folder change goes through the callbacks `App` gives
 * (the Documents one asks before it drops unsaved notes). A failure is handed to `onError`.
 */
export function useExplorerFolders({
  indexing,
  dataIndexing,
  onWorkFolderChosen,
  onDataFolderChosen,
  onError,
}: {
  indexing: IndexingState;
  dataIndexing: IndexingState;
  onWorkFolderChosen: (path: string) => void;
  onDataFolderChosen: (path: string) => void;
  onError: (error: AppError | null) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [resetting, setResetting] = useState(false);
  /* Counted up after an index is cleared, so the box reads its folder again: every file comes back
     as never analysed. */
  const [clears, setClears] = useState<Record<Family, number>>({ documents: 0, tables: 0 });

  const passOf = (family: Family) => (family === "documents" ? indexing : dataIndexing);
  const apply = (family: Family, path: string) => {
    passOf(family).reset();
    (family === "documents" ? onWorkFolderChosen : onDataFolderChosen)(path);
  };
  const report = (raw: unknown) => {
    const failure = normaliseError(raw);
    if (failure.code !== CANCELLED) {
      onError(failure);
    }
  };

  /* Every action that asks for a folder holds the others off while the dialog or the creation runs. */
  const guarded = async (action: () => Promise<void>) => {
    onError(null);
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };

  const choose = (family: Family) =>
    guarded(async () => {
      try {
        apply(family, await (family === "documents" ? chooseWorkFolder() : chooseDataFolder()));
      } catch (raw: unknown) {
        report(raw);
      }
    });

  const createSuggested = (family: Family) =>
    guarded(async () => {
      try {
        apply(
          family,
          await (family === "documents" ? ensureSuggestedWorkFolder() : ensureSuggestedDataFolder()),
        );
      } catch (raw: unknown) {
        report(raw);
      }
    });

  /* One after the other, each kept as soon as it is made: a failure on the second never loses the
     first, and the failure is said. */
  const createBoth = () =>
    guarded(async () => {
      const documents = await attemptFolder(ensureSuggestedWorkFolder, normaliseError);
      if (documents.path !== null) {
        apply("documents", documents.path);
      }
      const tables = await attemptFolder(ensureSuggestedDataFolder, normaliseError);
      if (tables.path !== null) {
        apply("tables", tables.path);
      }
      onError(documents.failure ?? tables.failure);
    });

  const reveal = async (family: Family) => {
    onError(null);
    try {
      await (family === "documents" ? revealWorkFolder() : revealDataFolder());
    } catch (raw: unknown) {
      onError(normaliseError(raw));
    }
  };

  /** Resolves to true when the analysis was cleared. */
  const clear = async (family: Family): Promise<boolean> => {
    onError(null);
    setResetting(true);
    try {
      await (family === "documents" ? resetIndex() : resetDataIndex());
      passOf(family).reset();
      setClears((now) => ({ ...now, [family]: now[family] + 1 }));
      return true;
    } catch (raw: unknown) {
      onError(normaliseError(raw));
      return false;
    } finally {
      setResetting(false);
    }
  };

  return { busy, resetting, clears, choose, createSuggested, createBoth, reveal, clear };
}
