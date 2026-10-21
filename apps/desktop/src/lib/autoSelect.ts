import {
  addEntriesToScopes,
  clearedScopes,
  type ScopesByFamily,
} from "./explorerSelection";
import { isTickable, type ExplorerEntry } from "./explorerModel";

/*
 * « Select. auto »: the optional switch that makes the selection follow the result of the filters
 * (owner specification, 18 October 2026; `docs/SELECTION-AND-MEMORY.md`, "Automatic selection"). Pure:
 * no React, no timer, no call, so that every rule is tested before any component exists.
 *
 * The rules, in the order the owner gave them:
 * - off by default and off at every launch (nothing here is stored);
 * - when on, the selection becomes the analysed files of the current result **after the result
 *   arrives**, never per keystroke (the caller sends `resultArrived` only for a settled result);
 * - turning it on records the selection as it was: **one** snapshot;
 * - a manual tick, untick or « Tout décocher » turns it off and drops nothing else (the snapshot stays);
 * - « Rétablir la sélection précédente » restores the snapshot and turns it off;
 * - the whole thing is locked while a question is being answered, like the selection: nothing changes
 *   under the answer, and a result that arrived meanwhile is applied when the question ends;
 * - an empty result gives an empty selection (« aucun »), which the bottom bar already warns about.
 */

/** The selection as it was when the switch was turned on, and how many files it used. */
export interface AutoSnapshot {
  scopes: ScopesByFamily;
  /** The files used on both sides, for the label of the restore link. */
  count: number;
}

/** A result to follow: the selection it gives and a key that is the same for the same result. */
export interface AutoTarget {
  scopes: ScopesByFamily;
  key: string;
}

export interface AutoState {
  on: boolean;
  snapshot: AutoSnapshot | null;
  /** A question is being answered. */
  locked: boolean;
  /** The last result followed, so the same result is not applied twice. */
  lastKey: string | null;
  /** A result that arrived while locked, followed when the question ends. */
  deferred: AutoTarget | null;
}

export const AUTO_OFF: AutoState = {
  on: false,
  snapshot: null,
  locked: false,
  lastKey: null,
  deferred: null,
};

export type AutoEvent =
  /** The user turned the switch on: the selection as it stands, and the result to follow now. */
  | { type: "switchOn"; scopes: ScopesByFamily; count: number; target: AutoTarget }
  | { type: "switchOff" }
  /** A settled result: the selection it gives. Never sent per keystroke. */
  | { type: "resultArrived"; target: AutoTarget }
  /** A manual tick, untick or « Tout décocher ». */
  | { type: "manualChange" }
  | { type: "restore" }
  | { type: "questionStarted" }
  | { type: "questionEnded" };

export interface AutoStep {
  state: AutoState;
  /** The selection to put in place now, or null when the selection stays as it is. */
  apply: ScopesByFamily | null;
}

const unchanged = (state: AutoState): AutoStep => ({ state, apply: null });

export function autoStep(state: AutoState, event: AutoEvent): AutoStep {
  switch (event.type) {
    case "switchOn":
      if (state.locked || state.on) {
        return unchanged(state);
      }
      return {
        state: {
          ...state,
          on: true,
          snapshot: { scopes: event.scopes, count: event.count },
          lastKey: event.target.key,
          deferred: null,
        },
        apply: event.target.scopes,
      };
    case "switchOff":
      if (state.locked || !state.on) {
        return unchanged(state);
      }
      return unchanged({ ...state, on: false, deferred: null });
    case "resultArrived":
      if (!state.on) {
        return unchanged(state);
      }
      if (state.locked) {
        return unchanged({ ...state, deferred: event.target });
      }
      if (event.target.key === state.lastKey) {
        return unchanged(state);
      }
      return { state: { ...state, lastKey: event.target.key }, apply: event.target.scopes };
    case "manualChange":
      if (state.locked || !state.on) {
        return unchanged(state);
      }
      return unchanged({ ...state, on: false, deferred: null });
    case "restore":
      if (state.locked || state.snapshot === null) {
        return unchanged(state);
      }
      return {
        state: { ...state, on: false, snapshot: null, deferred: null },
        apply: state.snapshot.scopes,
      };
    case "questionStarted":
      return unchanged({ ...state, locked: true });
    case "questionEnded": {
      const next = { ...state, locked: false, deferred: null };
      if (state.on && state.deferred !== null && state.deferred.key !== state.lastKey) {
        return {
          state: { ...next, lastKey: state.deferred.key },
          apply: state.deferred.scopes,
        };
      }
      return unchanged(next);
    }
  }
}

/** The selection a result gives: its analysed files, ticked in their own folder, and nothing else. A
 * side with none of them is « aucun »; a side whose every analysed file is in the result is « tous »,
 * as for any other ticking. */
export function targetOf(
  shown: readonly ExplorerEntry[],
  all: readonly ExplorerEntry[],
  now: number,
): AutoTarget {
  const scopes = addEntriesToScopes(clearedScopes(now), shown, all, now);
  const key = shown
    .filter(isTickable)
    .map((entry) => entry.key)
    .sort()
    .join("\n");
  return { scopes, key };
}

/** How many results cannot be ticked: the files of the result that are not analysed yet. The bar says
 * it while the switch is on. */
export function unreachableCount(shown: readonly ExplorerEntry[]): number {
  return shown.filter((entry) => !isTickable(entry)).length;
}

/** What the bar says of the mode, always. */
export type AutoMode = "automatic" | "manual";

export function modeOf(state: AutoState): AutoMode {
  return state.on ? "automatic" : "manual";
}

/** « Cocher les N résultats » is hidden while the switch is on: the selection follows the result. */
export function showsTickLink(state: AutoState, toTick: number): boolean {
  return !state.on && toTick > 0;
}

/** « Rétablir la sélection précédente (n) » shows while a snapshot exists. */
export function restoreCount(state: AutoState): number | null {
  return state.snapshot === null ? null : state.snapshot.count;
}
