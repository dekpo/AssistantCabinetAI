import { useCallback, useEffect, useRef, useState } from "react";
import { foundPaths, SEARCH_DELAY_MS, SEARCH_LIMIT } from "../lib/finderField";
import { findFiles, type FileRecord, type FinderDomain } from "../lib/ipc";

/**
 * The search field of a folder card: what she typed, the files the finder found, and whether the
 * listing shows all the files or only the ones found.
 *
 * Typing is her own action, so a finished search **replaces the selection** with the analysed files
 * it found (`onSelect`). Clearing the field, or "Réinitialiser", only ends the filter: the selection
 * stays as the search left it. The search runs after a pause in typing, never at launch, and costs
 * no model call (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 6.1).
 */
export interface FinderSearch {
  text: string;
  setText: (text: string) => void;
  /** The paths found, or null while no search is active. */
  found: string[] | null;
  /** The search could not run (the index is unavailable); the field says so. */
  failed: boolean;
  showAll: boolean;
  setShowAll: (showAll: boolean) => void;
  /** Ends the filter and shows the whole listing again. The selection is not touched. */
  reset: () => void;
}

export function useFinderSearch({
  domain,
  analysed,
  onSelect,
  enabled,
}: {
  domain: FinderDomain;
  /** The analysed files, for the selection the search makes. */
  analysed: FileRecord[];
  /** Called with the paths found, once a search has finished. */
  onSelect: (paths: string[], analysed: FileRecord[]) => void;
  /** False while the field is not shown (no folder, or a question is being answered). */
  enabled: boolean;
}): FinderSearch {
  const [text, setTextState] = useState("");
  const [found, setFound] = useState<string[] | null>(null);
  const [failed, setFailed] = useState(false);
  const [showAll, setShowAll] = useState(false);
  /* The latest values, read when a search ends, so that the effect below depends on the words she
     typed and on nothing else. */
  const latest = useRef({ analysed, onSelect });
  latest.current = { analysed, onSelect };
  const request = useRef(0);

  const setText = useCallback((next: string) => {
    setTextState(next);
    if (next.trim() === "") {
      // Clearing the field ends the filter and keeps the selection.
      request.current += 1;
      setFound(null);
      setFailed(false);
      setShowAll(false);
    }
  }, []);

  const reset = useCallback(() => setText(""), [setText]);

  useEffect(() => {
    const typed = text.trim();
    if (!enabled || typed === "") {
      return;
    }
    const mine = ++request.current;
    const timer = window.setTimeout(() => {
      findFiles(domain, { text: typed, limit: SEARCH_LIMIT, pathsOnly: true }).then(
        (result) => {
          if (mine !== request.current) {
            return; // A newer search, or a cleared field, has taken over.
          }
          const paths = foundPaths(result);
          setFound(paths);
          setFailed(false);
          latest.current.onSelect(paths, latest.current.analysed);
        },
        () => {
          if (mine === request.current) {
            setFound(null);
            setFailed(true);
          }
        },
      );
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [text, domain, enabled]);

  return { text, setText, found, failed, showAll, setShowAll, reset };
}
