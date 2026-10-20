import { SEARCH_DELAY_MS } from "./finderField";
import type { FinderReason } from "./ipc";

/**
 * What the search field of a folder card knows and decides, with no timer, no call and no React, so
 * that every rule below is tested without a browser (`finderSearchState.test.ts`). The hook
 * `useFinderSearch` is the thin shell that starts the request this state asks for and feeds the
 * answer back.
 *
 * The search only filters the view. Nothing here knows the selection, so no path through this module
 * can change it (`docs/DECISIONS.md`, "the left panel becomes one file explorer").
 *
 * Every request carries a number. An answer is kept only if it carries the number of the latest
 * request: typing again, clearing the field, a folder change or a switch to disabled all make the
 * answers in flight stale.
 */

/** A request the shell should start: after `delayMs`, search `text`, and answer with `id`. */
export interface WantedSearch {
  id: number;
  text: string;
  delayMs: number;
}

export interface SearchState {
  /** What she typed, as typed. */
  text: string;
  /** The paths found, or null while no search is active. */
  found: string[] | null;
  /** Why each path was found (`reasons[path]`); empty while no search is active. */
  reasons: Record<string, FinderReason[]>;
  /** The extensions the finder took out of the typed text and applied as a format filter. */
  typedFormats: string[];
  /** The search could not run; the field says so. */
  failed: boolean;
  showAll: boolean;
  /** False while the field is not shown (no folder, or a question is being answered). */
  enabled: boolean;
  /** The number of the latest request; an answer with another number is dropped. */
  request: number;
  /** The request to start now, or null. Replaced by every change that asks for a new search. */
  wanted: WantedSearch | null;
}

export type SearchAction =
  | { type: "textChanged"; text: string }
  /** The folder of the card changed: the text and the results belong to the old folder. */
  | { type: "folderChanged" }
  | { type: "enabledChanged"; enabled: boolean }
  /** A pass finished or the inventory was read again: the files may have changed under the text. */
  | { type: "refresh" }
  | {
      type: "answered";
      id: number;
      paths: string[];
      reasons?: Record<string, FinderReason[]>;
      typedFormats?: string[];
    }
  | { type: "failed"; id: number }
  | { type: "showAllChanged"; showAll: boolean };

export function initialSearchState(enabled: boolean): SearchState {
  return {
    text: "",
    found: null,
    reasons: {},
    typedFormats: [],
    failed: false,
    showAll: false,
    enabled,
    request: 0,
    wanted: null,
  };
}

/** No search is active and none is asked for. */
function cleared(state: SearchState, text: string): SearchState {
  return {
    ...state,
    text,
    found: null,
    reasons: {},
    typedFormats: [],
    failed: false,
    showAll: false,
    request: state.request + 1,
    wanted: null,
  };
}

/** A new request for the text as it stands now; whatever was in flight becomes stale. The results
 * on screen stay until the answer replaces them, so the list does not blink between two requests. */
function asking(state: SearchState, delayMs: number): SearchState {
  const text = state.text.trim();
  const id = state.request + 1;
  if (!state.enabled || text === "") {
    return { ...state, request: id, wanted: null };
  }
  return { ...state, request: id, wanted: { id, text, delayMs } };
}

export function searchReducer(state: SearchState, action: SearchAction): SearchState {
  switch (action.type) {
    case "textChanged": {
      if (action.text.trim() === "") {
        // Clearing the field ends the filter and shows the whole listing.
        return cleared(state, action.text);
      }
      // Typing waits for a pause, so a result per keystroke does not flicker the list.
      return asking({ ...state, text: action.text }, SEARCH_DELAY_MS);
    }
    case "folderChanged": {
      if (state.text === "" && state.found === null && !state.failed && state.wanted === null) {
        return state;
      }
      return cleared(state, "");
    }
    case "enabledChanged": {
      if (action.enabled === state.enabled) {
        return state;
      }
      const next = { ...state, enabled: action.enabled };
      // Switched off: an answer still in flight describes a field that is no longer in use. Switched
      // on again: the files may have changed meanwhile, search again at once.
      return asking(next, 0);
    }
    case "refresh": {
      if (state.text.trim() === "" || !state.enabled) {
        return state;
      }
      return asking(state, 0);
    }
    case "answered": {
      if (action.id !== state.request) {
        return state;
      }
      return {
        ...state,
        found: action.paths,
        reasons: action.reasons ?? {},
        typedFormats: action.typedFormats ?? [],
        failed: false,
        wanted: null,
      };
    }
    case "failed": {
      if (action.id !== state.request) {
        return state;
      }
      return { ...state, found: null, reasons: {}, typedFormats: [], failed: true, wanted: null };
    }
    case "showAllChanged": {
      return { ...state, showAll: action.showAll };
    }
  }
}
