import type { NoteFormat } from "../speech/ipc";
import { timestampParts } from "./timestamp";

/**
 * The notes being written, as plain data. They live in memory only (decision D10: no recovery copy
 * on disk), owned by `App` through the notes session, so a switch of section or a hidden centre
 * never loses them. Every function here is pure and returns a new state.
 */

export interface DraftNote {
  id: string;
  /** Position in the order of creation; the label of a draft without a name ("Note 3"). */
  number: number;
  /** What the user typed for the file name; may be empty (the file is then named by its time). */
  name: string;
  text: string;
  format: NoteFormat;
}

export interface NotesState {
  drafts: DraftNote[];
  activeId: string;
  /** How many drafts were ever created in this window: it only keeps the ids unique. */
  created: number;
}

/** `number` is what the user reads ("Note 2"); `created` only keeps the ids unique. */
function blankDraft(created: number, number: number): DraftNote {
  return { id: `note-${created}`, number, name: "", text: "", format: "md" };
}

/** One empty note, selected. */
export function initialNotes(): NotesState {
  const first = blankDraft(1, 1);
  return { drafts: [first], activeId: first.id, created: 1 };
}

/** A note counts only once something besides white space was typed. */
export function hasContent(draft: DraftNote): boolean {
  return draft.text.trim() !== "";
}

/** The drafts a save would write, in order. */
export function unsavedDrafts(state: NotesState): DraftNote[] {
  return state.drafts.filter(hasContent);
}

export function hasUnsaved(state: NotesState): boolean {
  return state.drafts.some(hasContent);
}

export function activeDraft(state: NotesState): DraftNote {
  return state.drafts.find((draft) => draft.id === state.activeId) ?? state.drafts[0]!;
}

/** A new empty note, selected and placed last. */
export function addDraft(state: NotesState): NotesState {
  const number = Math.max(...state.drafts.map((entry) => entry.number)) + 1;
  const draft = blankDraft(state.created + 1, number);
  return { drafts: [...state.drafts, draft], activeId: draft.id, created: state.created + 1 };
}

export function selectDraft(state: NotesState, id: string): NotesState {
  return state.drafts.some((draft) => draft.id === id) && id !== state.activeId
    ? { ...state, activeId: id }
    : state;
}

export type DraftPatch = Partial<Pick<DraftNote, "name" | "text" | "format">>;

export function editDraft(state: NotesState, id: string, patch: DraftPatch): NotesState {
  let changed = false;
  const drafts = state.drafts.map((draft) => {
    if (draft.id !== id) {
      return draft;
    }
    const next = { ...draft, ...patch };
    changed = next.name !== draft.name || next.text !== draft.text || next.format !== draft.format;
    return changed ? next : draft;
  });
  return changed ? { ...state, drafts } : state;
}

/**
 * Takes drafts out of the list. The list is never left empty: one fresh blank note takes the place
 * of the last one, and the selection moves to the neighbour of a removed active draft.
 */
export function removeDrafts(state: NotesState, ids: readonly string[]): NotesState {
  const gone = new Set(ids);
  if (!state.drafts.some((draft) => gone.has(draft.id))) {
    return state;
  }
  const activeIndex = state.drafts.findIndex((draft) => draft.id === state.activeId);
  const drafts = state.drafts.filter((draft) => !gone.has(draft.id));
  if (drafts.length === 0) {
    // Nothing left to number: the list starts again at "Note 1".
    const fresh = blankDraft(state.created + 1, 1);
    return { drafts: [fresh], activeId: fresh.id, created: state.created + 1 };
  }
  if (drafts.some((draft) => draft.id === state.activeId)) {
    return { ...state, drafts };
  }
  const neighbour = drafts[Math.min(activeIndex, drafts.length - 1)]!;
  return { ...state, drafts, activeId: neighbour.id };
}

/** Everything thrown away at once: one blank note, as at the first launch. */
export function discardAll(state: NotesState): NotesState {
  return removeDrafts(state, state.drafts.map((draft) => draft.id));
}

/**
 * A draft is clean once its file is written. What was written is compared with what the draft says
 * now: a draft edited after the plan was made stays, with its newer text.
 */
export function markWritten(
  state: NotesState,
  written: ReadonlyArray<{ id: string; text: string }>,
): NotesState {
  const clean = written
    .filter((entry) => state.drafts.some((draft) => draft.id === entry.id && draft.text === entry.text))
    .map((entry) => entry.id);
  return removeDrafts(state, clean);
}

/** The label of a draft in the switcher: its name when it has one, else "Note <number>" (the caller
 * puts the number through the catalogue). `null` means "use the numbered label". */
export function draftTitle(draft: DraftNote): string | null {
  const name = draft.name.trim();
  return name === "" ? null : name;
}

/** `YYYY-MM-DD_HH-MM-SS`, the name Rust gives a note without a name: shown as the field's hint. */
export function stampName(milliseconds: number): string {
  const part = timestampParts(milliseconds);
  return `${part.year}-${part.month}-${part.day}_${part.hour}-${part.minute}-${part.second}`;
}

/** The ways out of the Notes section that exist in the window. */
export type NotesExit =
  /** Files | Notes switch: both stay mounted, nothing is lost. */
  | { kind: "switch-section" }
  /** A file opened from a list: the Files section is shown, Notes stays alive. */
  | { kind: "open-file" }
  /** The centre hidden (title-bar button, drag of the separator): drafts stay in memory, but a
   * recording in progress would be destroyed with the section. */
  | { kind: "hide-zone" }
  | { kind: "change-folder" }
  | { kind: "close-window" }
  | { kind: "disable-notes" }
  | { kind: "reset-settings" }
  | { kind: "discard-draft"; id: string };

export type ExitDecision = "allow" | "ask";

/**
 * The one place that decides whether leaving needs a question: "ask" exactly when something the
 * user wrote would be lost. A modal then waits for the user; nothing is thrown away before.
 */
export function guardExit(
  state: NotesState,
  exit: NotesExit,
  context: { recording?: boolean } = {},
): ExitDecision {
  switch (exit.kind) {
    case "switch-section":
    case "open-file":
      return "allow";
    case "hide-zone":
      return context.recording === true ? "ask" : "allow";
    case "discard-draft": {
      const draft = state.drafts.find((entry) => entry.id === exit.id);
      return draft !== undefined && hasContent(draft) ? "ask" : "allow";
    }
    case "change-folder":
    case "close-window":
    case "disable-notes":
    case "reset-settings":
      return hasUnsaved(state) ? "ask" : "allow";
  }
}
