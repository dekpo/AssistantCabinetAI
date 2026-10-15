import { useCallback, useEffect, useRef, useState } from "react";
import { foundPaths, SEARCH_DELAY_MS, SEARCH_LIMIT } from "../lib/finderField";
import { applyFilters, hasFilters, NO_FILTERS, toggleFilter, type FacetGroup, type FinderFilters } from "../lib/finderFilters";
import { findFiles, userTypesList, type FileRecord, type FinderDomain, type FinderFacets } from "../lib/ipc";

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
  /** The chips chosen over the facets of the result, and the facets themselves. */
  filters: FinderFilters;
  toggle: (group: FacetGroup, value: string) => void;
  facets: FinderFacets | null;
  /** The names the user gave their own business types, by code. */
  userTypeLabels: Record<string, string>;
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
  const [filters, setFilters] = useState<FinderFilters>(NO_FILTERS);
  const [facets, setFacets] = useState<FinderFacets | null>(null);
  const [userTypeLabels, setUserTypeLabels] = useState<Record<string, string>>({});
  /* The latest values, read when a search ends, so that the effect below depends on the words she
     typed and on nothing else. */
  const latest = useRef({ analysed, onSelect, filters: NO_FILTERS });
  latest.current = { analysed, onSelect, filters };
  const request = useRef(0);

  /* Without words and without chips there is no search: the filter ends and the selection stays. */
  const end = useCallback(() => {
    request.current += 1;
    setFound(null);
    setFacets(null);
    setFailed(false);
    setShowAll(false);
  }, []);

  const setText = useCallback(
    (next: string) => {
      setTextState(next);
      if (next.trim() === "" && !hasFilters(latest.current.filters)) {
        end();
      }
    },
    [end],
  );

  const toggle = useCallback(
    (group: FacetGroup, value: string) => setFilters((current) => toggleFilter(current, group, value)),
    [],
  );

  const reset = useCallback(() => {
    setTextState("");
    setFilters(NO_FILTERS);
    end();
  }, [end]);

  useEffect(() => {
    const typed = text.trim();
    if (!enabled || (typed === "" && !hasFilters(filters))) {
      if (typed === "" && !hasFilters(filters)) {
        end();
      }
      return;
    }
    const mine = ++request.current;
    const timer = window.setTimeout(() => {
      const query = applyFilters(
        { text: typed === "" ? null : typed, limit: SEARCH_LIMIT, pathsOnly: true },
        filters,
      );
      findFiles(domain, query).then(
        (result) => {
          if (mine !== request.current) {
            return; // A newer search, or a cleared field, has taken over.
          }
          const paths = foundPaths(result);
          setFound(paths);
          setFacets(result.facets);
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
  }, [text, filters, domain, enabled, end]);

  /* The names of the user's own types, read once when a search first runs (one cheap lookup). */
  const wantsLabels = found !== null;
  useEffect(() => {
    if (!wantsLabels) {
      return;
    }
    userTypesList().then(
      (types) => setUserTypeLabels(Object.fromEntries(types.map((one) => [one.code, one.label]))),
      () => undefined,
    );
  }, [wantsLabels]);

  return {
    text,
    setText,
    found,
    failed,
    showAll,
    setShowAll,
    filters,
    toggle,
    facets,
    userTypeLabels,
    reset,
  };
}
