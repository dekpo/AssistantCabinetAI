import { describe, expect, it } from "vitest";
import {
  activeDraft,
  addDraft,
  discardAll,
  draftTitle,
  editDraft,
  guardExit,
  hasUnsaved,
  initialNotes,
  markWritten,
  removeDrafts,
  selectDraft,
  stampName,
  unsavedDrafts,
  type NotesExit,
  type NotesState,
} from "./notesDrafts";

function withText(...texts: string[]): NotesState {
  let state = initialNotes();
  texts.forEach((text, index) => {
    if (index > 0) {
      state = addDraft(state);
    }
    state = editDraft(state, state.activeId, { text });
  });
  return state;
}

describe("the drafts", () => {
  it("start with one empty note, selected, that is not worth saving", () => {
    const state = initialNotes();

    expect(state.drafts).toHaveLength(1);
    expect(activeDraft(state).format).toBe("md");
    expect(hasUnsaved(state)).toBe(false);
    expect(unsavedDrafts(state)).toStrictEqual([]);
  });

  it("count a note only once something besides white space was typed", () => {
    expect(hasUnsaved(withText("  \n\t "))).toBe(false);
    expect(hasUnsaved(withText("Bonjour"))).toBe(true);
  });

  it("add a note at the end and select it, numbering keeps counting", () => {
    const state = addDraft(addDraft(initialNotes()));

    expect(state.drafts.map((draft) => draft.number)).toStrictEqual([1, 2, 3]);
    expect(state.activeId).toBe(state.drafts[2]!.id);
  });

  it("select a note by id and ignore an id that does not exist", () => {
    const state = addDraft(initialNotes());
    const first = state.drafts[0]!.id;

    expect(selectDraft(state, first).activeId).toBe(first);
    expect(selectDraft(state, "nope")).toBe(state);
  });

  it("edit one note without touching the others, and return the same state when nothing changed", () => {
    const state = withText("un", "deux");
    const first = state.drafts[0]!;

    const edited = editDraft(state, first.id, { name: "Visite", format: "txt" });

    expect(edited.drafts[0]).toMatchObject({ name: "Visite", format: "txt", text: "un" });
    expect(edited.drafts[1]).toBe(state.drafts[1]);
    expect(editDraft(edited, first.id, { name: "Visite" })).toBe(edited);
  });

  it("number again from 1 once every note was saved or thrown away", () => {
    const state = withText("un", "deux");

    const next = markWritten(state, [
      { id: "note-1", text: "un" },
      { id: "note-2", text: "deux" },
    ]);

    expect(next.drafts.map((draft) => draft.number)).toStrictEqual([1]);
    expect(next.drafts[0]!.id).toBe("note-3");
    expect(addDraft(next).drafts.map((draft) => draft.number)).toStrictEqual([1, 2]);
  });

  it("never end up empty: the last note removed is replaced by a fresh one", () => {
    const state = withText("un");

    const next = removeDrafts(state, [state.activeId]);

    expect(next.drafts).toHaveLength(1);
    expect(next.drafts[0]!.text).toBe("");
    expect(next.drafts[0]!.id).not.toBe(state.activeId);
  });

  it("move the selection to a neighbour when the selected note is removed", () => {
    const state = selectDraft(withText("un", "deux", "trois"), "note-2");

    const next = removeDrafts(state, ["note-2"]);

    expect(next.drafts.map((draft) => draft.text)).toStrictEqual(["un", "trois"]);
    expect(next.activeId).toBe("note-3");
  });

  it("keep the selection when another note is removed", () => {
    const state = withText("un", "deux");

    expect(removeDrafts(state, ["note-1"]).activeId).toBe("note-2");
  });

  it("throw everything away at once", () => {
    const next = discardAll(withText("un", "deux"));

    expect(next.drafts).toHaveLength(1);
    expect(hasUnsaved(next)).toBe(false);
  });

  it("clean the notes whose file was written", () => {
    const state = withText("un", "deux");

    const next = markWritten(state, [{ id: "note-1", text: "un" }]);

    expect(next.drafts.map((draft) => draft.text)).toStrictEqual(["deux"]);
  });

  it("keep a note edited after the plan was made, with its newer text", () => {
    const state = withText("un");
    const edited = editDraft(state, "note-1", { text: "un, puis la suite" });

    expect(markWritten(edited, [{ id: "note-1", text: "un" }]).drafts[0]!.text).toBe(
      "un, puis la suite",
    );
  });

  it("title a note by its name when it has one, else by nothing (the numbered label)", () => {
    const state = withText("un");

    expect(draftTitle(state.drafts[0]!)).toBeNull();
    expect(draftTitle(editDraft(state, "note-1", { name: "  Visite  " }).drafts[0]!)).toBe("Visite");
  });

  it("show the time a note without a name would carry", () => {
    expect(stampName(new Date(2026, 9, 18, 9, 5, 7).getTime())).toBe("2026-10-18_09-05-07");
  });
});

describe("every way out of the Notes section goes through guardExit", () => {
  const exits: Array<[string, NotesExit, "allow" | "ask"]> = [
    ["switching section", { kind: "switch-section" }, "allow"],
    ["opening a file from a list", { kind: "open-file" }, "allow"],
    ["hiding the zone", { kind: "hide-zone" }, "allow"],
    ["choosing another folder", { kind: "change-folder" }, "ask"],
    ["the window close request", { kind: "close-window" }, "ask"],
    ["turning the Notes switch off", { kind: "disable-notes" }, "ask"],
    ["resetting the settings", { kind: "reset-settings" }, "ask"],
    ["discarding a note with text", { kind: "discard-draft", id: "note-1" }, "ask"],
  ];

  it.each(exits)("%s, with an unsaved note", (_name, exit, expected) => {
    expect(guardExit(withText("Bonjour"), exit)).toBe(expected);
  });

  it.each(exits)("%s, with nothing to lose, never asks", (_name, exit) => {
    expect(guardExit(initialNotes(), exit)).toBe("allow");
  });

  it("never asks for the section switch or for opening a file, whatever the notes hold", () => {
    const state = withText("un", "deux");

    expect(guardExit(state, { kind: "switch-section" }, { recording: true })).toBe("allow");
    expect(guardExit(state, { kind: "open-file" }, { recording: true })).toBe("allow");
  });

  it("asks before hiding the zone only while a recording would be destroyed", () => {
    expect(guardExit(withText("Bonjour"), { kind: "hide-zone" })).toBe("allow");
    expect(guardExit(initialNotes(), { kind: "hide-zone" }, { recording: true })).toBe("ask");
  });

  it("asks about discarding one note only when that note has text", () => {
    const state = addDraft(withText("un"));

    expect(guardExit(state, { kind: "discard-draft", id: "note-1" })).toBe("ask");
    expect(guardExit(state, { kind: "discard-draft", id: "note-2" })).toBe("allow");
    expect(guardExit(state, { kind: "discard-draft", id: "missing" })).toBe("allow");
  });
});
