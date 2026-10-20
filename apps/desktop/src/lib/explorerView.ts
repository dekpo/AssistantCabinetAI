import type { PanelChoices } from "./explorerFilters";
import { NO_CHOICES } from "./explorerFilters";
import type { ExplorerEntry, ExplorerState, Family } from "./explorerModel";

/*
 * What narrows the explorer's list apart from the typed text: the segmented control's family and the
 * filter panel's choices. Held by `App`, so that a link in the centre (the overview's « Afficher les N
 * dans la liste de gauche ») sets the same state the panel sets, and a filter is written once. None of
 * this is the selection: a filter only narrows what is drawn.
 */

/** The family filter of the segmented control: everything, or one family. */
export type FamilyChoice = "all" | Family;

export interface ExplorerView {
  family: FamilyChoice;
  choices: PanelChoices;
}

export const NO_VIEW: ExplorerView = { family: "all", choices: NO_CHOICES };

/**
 * The view that lists only the files in one state, in one family or in both: what the overview's
 * link asks for. Format and folder choices are dropped, because they could hide some of the files the
 * link counted; the typed text is cleared by the caller (it is not part of this state).
 */
export function viewShowingState(state: ExplorerState, family: FamilyChoice = "all"): ExplorerView {
  return {
    family,
    choices: { formats: new Set(), states: new Set([state]), folders: new Set() },
  };
}

/**
 * The folded lists that « Afficher les N dans la liste de gauche » opens: only a list that holds at
 * least one of the files in that state, and only if it is folded. It is the one place the application
 * opens a list for the user, as the consequence of the link she pressed; a filter pill removed, a
 * filter changed or a pass finishing never opens or closes a list.
 */
export function familiesToUnfold(
  entries: readonly ExplorerEntry[],
  state: ExplorerState,
  folded: Readonly<Record<Family, boolean>>,
): Family[] {
  const holding = new Set(entries.filter((entry) => entry.state === state).map((e) => e.family));
  return (["documents", "tables"] as const).filter((family) => folded[family] && holding.has(family));
}

/**
 * The lists the link opened and that close again: when the « unreadable » filter it set is no longer
 * among the choices (its pill was removed, or the filters cleared). Anything else the user changes
 * leaves them as they are, and a fold she toggled herself is forgotten by the caller (`opened` is
 * emptied), so the application never closes a list she opened or closed herself.
 */
export function foldsToRestore(opened: readonly Family[], view: ExplorerView): Family[] {
  return view.choices.states.has("unreadable") ? [] : [...opened];
}
