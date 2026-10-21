import { NO_CHOICES, chosenEntityIds } from "./explorerFilters";
import type { ExplorerState } from "./explorerModel";
import type { ExplorerView } from "./explorerView";
import type { SavedViewSetting } from "./ipc";

/*
 * The saved views of the explorer, with no React and no call. A view is **filters only**: the family
 * and the choices of the panel (formats, states, origin folders, people, organisations, business types,
 * years, the newest-first order, the conversation's files). It never holds a file, a selection or the
 * typed search text; opening one replaces the current filters and never touches the selection, the
 * folds or the typed text (`docs/DECISIONS.md`, "Explorer 7"). A person or an organisation is kept by
 * its id, never by name, so that no name is written into the settings file.
 */

/** The most views the user can save; `settings.rs` keeps the same number. */
export const MAX_SAVED_VIEWS = 50;
/** The longest name of a view, in characters; `settings.rs` cuts at the same length. */
export const MAX_VIEW_NAME = 60;

/** The views the application brings, with the catalogue key of their name (`explorer.builtin.<id>`). */
export type BuiltInViewId = "recent" | "toAnalyse" | "inConversation";

export const BUILT_IN_VIEWS: readonly { id: BuiltInViewId; view: ExplorerView }[] = [
  { id: "recent", view: { family: "all", choices: { ...NO_CHOICES, recent: true } } },
  {
    id: "toAnalyse",
    view: { family: "all", choices: { ...NO_CHOICES, states: new Set<ExplorerState>(["toAnalyse"]) } },
  },
  {
    id: "inConversation",
    view: { family: "all", choices: { ...NO_CHOICES, inConversation: true } },
  },
];

const STATES: readonly ExplorerState[] = ["analysed", "toAnalyse", "unreadable"];

/** The view a saved one opens. */
export function viewFromSaved(saved: SavedViewSetting): ExplorerView {
  return {
    family: saved.family,
    choices: {
      formats: new Set(saved.formats),
      states: new Set(saved.states.filter((state) => STATES.includes(state))),
      folders: new Set(saved.folders),
      people: new Set(saved.people),
      organisations: new Set(saved.organisations),
      types: new Set(saved.types),
      years: new Set(saved.years),
      recent: saved.recent,
      inConversation: saved.inConversation,
    },
  };
}

/** The view as it is saved. Sets are written in a stable order. */
export function savedFromView(view: ExplorerView, id: string, name: string): SavedViewSetting {
  const sorted = (set: ReadonlySet<string>) => [...set].sort();
  return {
    id,
    name: cleanViewName(name),
    family: view.family,
    formats: sorted(view.choices.formats),
    states: STATES.filter((state) => view.choices.states.has(state)),
    folders: sorted(view.choices.folders),
    people: sorted(view.choices.people),
    organisations: sorted(view.choices.organisations),
    types: sorted(view.choices.types),
    years: sorted(view.choices.years),
    recent: view.choices.recent,
    inConversation: view.choices.inConversation,
  };
}

/** What was typed as a name: trimmed, inner runs of spaces made one, cut at the limit. */
export function cleanViewName(typed: string): string {
  return typed.replace(/\s+/g, " ").trim().slice(0, MAX_VIEW_NAME);
}

/** Whether the filters hold nothing: there is nothing worth saving. */
export function isEmptyView(view: ExplorerView): boolean {
  const { choices } = view;
  return (
    view.family === "all" &&
    choices.formats.size === 0 &&
    choices.states.size === 0 &&
    choices.folders.size === 0 &&
    choices.people.size === 0 &&
    choices.organisations.size === 0 &&
    choices.types.size === 0 &&
    choices.years.size === 0 &&
    !choices.recent &&
    !choices.inConversation
  );
}

/** Whether two views hold the same filters, in whatever order they were chosen. */
export function sameView(a: ExplorerView, b: ExplorerView): boolean {
  return JSON.stringify(savedFromView(a, "", "")) === JSON.stringify(savedFromView(b, "", ""));
}

/** Whether another saved view already has this name (case and accents ignored). */
export function nameTaken(
  views: readonly SavedViewSetting[],
  typed: string,
  exceptId?: string,
): boolean {
  const plain = (text: string) =>
    cleanViewName(text)
      .normalize("NFD")
      .replace(/\p{M}/gu, "")
      .toLowerCase();
  const wanted = plain(typed);
  return views.some((view) => view.id !== exceptId && plain(view.name) === wanted);
}

/** An id no saved view has. */
export function newViewId(views: readonly SavedViewSetting[], now: number): string {
  const taken = new Set(views.map((view) => view.id));
  let id = `v${now.toString(36)}`;
  for (let suffix = 2; taken.has(id); suffix += 1) {
    id = `v${now.toString(36)}-${suffix}`;
  }
  return id;
}

/** Why a name cannot be saved, or null when it can. */
export type NameProblem = "empty" | "taken";

export function nameProblem(
  views: readonly SavedViewSetting[],
  typed: string,
  exceptId?: string,
): NameProblem | null {
  if (cleanViewName(typed) === "") {
    return "empty";
  }
  return nameTaken(views, typed, exceptId) ? "taken" : null;
}

/** The views with one added, or the same list when the name cannot be used or the list is full. */
export function addView(views: readonly SavedViewSetting[], saved: SavedViewSetting): SavedViewSetting[] {
  if (views.length >= MAX_SAVED_VIEWS || nameProblem(views, saved.name) !== null) {
    return [...views];
  }
  return [...views, saved];
}

/** The views with one renamed, or the same list when the name cannot be used. */
export function renameView(
  views: readonly SavedViewSetting[],
  id: string,
  typed: string,
): SavedViewSetting[] {
  if (nameProblem(views, typed, id) !== null) {
    return [...views];
  }
  return views.map((view) => (view.id === id ? { ...view, name: cleanViewName(typed) } : view));
}

export function deleteView(views: readonly SavedViewSetting[], id: string): SavedViewSetting[] {
  return views.filter((view) => view.id !== id);
}

/**
 * The people and organisations of a view that the files no longer link to: their ids. The caller asks
 * only once the facts of every chosen folder have been read, so that a name is never called missing
 * for a read that has not happened.
 */
export function missingEntities(view: ExplorerView, known: ReadonlySet<number>): number[] {
  const ids = new Set([
    ...chosenEntityIds(view.choices.people),
    ...chosenEntityIds(view.choices.organisations),
  ]);
  return [...ids].filter((id) => !known.has(id)).sort((a, b) => a - b);
}
