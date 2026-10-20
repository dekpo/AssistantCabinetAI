import { useEffect, useMemo, useState } from "react";
import { joinFacts, NO_FACTS, type KnowledgeFacts } from "../lib/explorerFacts";
import { fileKnowledgeFacts, type FinderDomain, type FolderKnowledgeFacts } from "../lib/ipc";

/**
 * What the knowledge base knows of the files of the two folders, for the explorer's filters: read once
 * per folder when something shows or uses it, and again after each pass and each time the panel is
 * opened, so that a name the user merged or deleted in the knowledge base is not offered any more.
 * Never at launch, never while nothing needs it (`wanted` false): reading is cheap (about 55 ms for
 * 1,250 files, measured 20 October 2026) but the application starts nothing it does not need.
 *
 * A read that fails leaves its folder without facts: the filters of the knowledge base then offer
 * nothing for it, and the rest of the panel works as before.
 */
export interface KnowledgeFactsState {
  facts: KnowledgeFacts;
  /** The facts of every chosen folder have been read at least once since `wanted` became true. */
  loaded: boolean;
}

type Read = Record<FinderDomain, FolderKnowledgeFacts | null>;

export function useKnowledgeFacts({
  documentsFolder,
  dataFolder,
  documentsPasses,
  dataPasses,
  wanted,
  refresh,
}: {
  documentsFolder: string | null;
  dataFolder: string | null;
  /** Counted up by every finished pass of each folder: the profile may have changed. */
  documentsPasses: number;
  dataPasses: number;
  /** Something shows or uses the facts now. */
  wanted: boolean;
  /** Changes when the facts should be read again although nothing else changed (the panel opening). */
  refresh: unknown;
}): KnowledgeFactsState {
  const [read, setRead] = useState<Read>({ documents: null, data: null });
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (!wanted) {
      return;
    }
    let current = true;
    const ask = (domain: FinderDomain, folder: string | null) =>
      folder === null
        ? Promise.resolve<FolderKnowledgeFacts | null>(null)
        : fileKnowledgeFacts(domain).catch(() => null);
    void Promise.all([ask("documents", documentsFolder), ask("data", dataFolder)]).then(
      ([documents, data]) => {
        if (current) {
          setRead({ documents, data });
          setLoaded(true);
        }
      },
    );
    return () => {
      current = false;
    };
  }, [wanted, documentsFolder, dataFolder, documentsPasses, dataPasses, refresh]);

  /* A folder that changed is read again: what the old one said no longer applies. */
  useEffect(() => {
    setLoaded(false);
    setRead({ documents: null, data: null });
  }, [documentsFolder, dataFolder]);

  const facts = useMemo(
    () => (read.documents === null && read.data === null ? NO_FACTS : joinFacts(read)),
    [read],
  );
  return { facts, loaded: wanted && loaded };
}
