import { useCallback, useEffect, useReducer } from "react";
import { foundPaths, SEARCH_LIMIT } from "../lib/finderField";
import { initialSearchState, searchReducer } from "../lib/finderSearchState";
import { findFiles, type FinderDomain } from "../lib/ipc";

/**
 * The search field of a folder card: what she typed, the files the finder found, and whether the
 * listing shows all the files or only the ones found.
 *
 * A search only filters the view and never touches the selection: ticking the results is a separate
 * click (`FinderControls`). The rules (pause in typing, stale answers, a new search after a pass)
 * live in `lib/finderSearchState.ts`, which is tested; this hook only starts the request that state
 * asks for and gives the answer back. The search runs after a pause in typing, never at launch, and
 * costs no model call (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 6.1).
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
  folder,
  finishedPasses,
  inventory,
  enabled,
}: {
  domain: FinderDomain;
  /** The folder of the card: the text and the results are dropped when it changes. */
  folder: string | null;
  /** Counted up by every finished pass: the files and their names may have changed. */
  finishedPasses: number;
  /** The inventory last read (it changes identity at every read, including on window focus). */
  inventory: unknown;
  /** False while the field is not shown (no folder, or a question is being answered). */
  enabled: boolean;
}): FinderSearch {
  const [state, dispatch] = useReducer(searchReducer, enabled, initialSearchState);

  const setText = useCallback((text: string) => dispatch({ type: "textChanged", text }), []);
  const setShowAll = useCallback(
    (showAll: boolean) => dispatch({ type: "showAllChanged", showAll }),
    [],
  );
  const reset = useCallback(() => dispatch({ type: "textChanged", text: "" }), []);

  useEffect(() => dispatch({ type: "folderChanged" }), [folder]);
  useEffect(() => dispatch({ type: "enabledChanged", enabled }), [enabled]);
  useEffect(() => dispatch({ type: "refresh" }), [finishedPasses, inventory]);

  const wanted = state.wanted;
  useEffect(() => {
    if (wanted === null) {
      return;
    }
    const timer = window.setTimeout(() => {
      findFiles(domain, { text: wanted.text, limit: SEARCH_LIMIT, pathsOnly: true }).then(
        (result) => dispatch({ type: "answered", id: wanted.id, paths: foundPaths(result) }),
        () => dispatch({ type: "failed", id: wanted.id }),
      );
    }, wanted.delayMs);
    return () => window.clearTimeout(timer);
  }, [wanted, domain]);

  return {
    text: state.text,
    setText,
    found: state.found,
    failed: state.failed,
    showAll: state.showAll,
    setShowAll,
    reset,
  };
}
