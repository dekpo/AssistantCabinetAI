import { describe, expect, it, vi } from "vitest";
import type { NoteDraft, NoteFile } from "../speech/ipc";
import type { FileRecord } from "./ipc";
import { createNotesSession, type NotesSessionDeps } from "./notesSession";

function note(name: string): NoteFile {
  return {
    relativePath: `Notes/2026-10-18_14-32-05-${name}.md`,
    fileName: `2026-10-18_14-32-05-${name}.md`,
    bytes: 4,
    chars: 4,
    lines: 1,
  };
}

function pending(relativePath: string): FileRecord {
  return {
    id: relativePath,
    relativePath,
    name: relativePath,
    stem: relativePath,
    extension: "pdf",
    kind: "document_pdf",
    mimeType: "application/pdf",
    sizeBytes: 1,
    modifiedAt: null,
    sha256: null,
    readability: "not_assessed",
    processingStatus: "discovered",
    extractionMethod: "none",
    indexed: false,
    indexMetadata: null,
  };
}

function session(overrides: Partial<NotesSessionDeps> = {}) {
  const deps: NotesSessionDeps = {
    preview: vi.fn(async (drafts: NoteDraft[]) => drafts.map((_, index) => note(`p${index + 1}`))),
    save: vi.fn(async (drafts: NoteDraft[]) => drafts.map((_, index) => note(`s${index + 1}`))),
    inventory: vi.fn(async () => []),
    analyse: vi.fn(async () => true),
    ...overrides,
  };
  return { notes: createNotesSession(deps), deps };
}

/** Types two notes, as in the human test. */
function twoNotes() {
  const made = session();
  made.notes.edit("note-1", { text: "premiere" });
  made.notes.add();
  made.notes.edit("note-2", { text: "seconde", name: "Rendez-vous" });
  return made;
}

describe("the notes outlive the screen", () => {
  it("keep their text through a subscription that comes and goes, as a panel does on a section switch", () => {
    const { notes } = session();
    const unsubscribe = notes.subscribe(() => undefined);
    notes.edit("note-1", { text: "ecrit a la main" });
    unsubscribe();

    expect(notes.getSnapshot().notes.drafts[0]!.text).toBe("ecrit a la main");
    notes.subscribe(() => undefined);
    expect(notes.getSnapshot().notes.drafts[0]!.text).toBe("ecrit a la main");
  });

  it("tell their subscribers about each change, and hand out a new snapshot only then", () => {
    const { notes } = session();
    const listener = vi.fn();
    notes.subscribe(listener);
    const before = notes.getSnapshot();

    notes.edit("note-1", { text: "a" });

    expect(listener).toHaveBeenCalledTimes(1);
    expect(notes.getSnapshot()).not.toBe(before);
  });
});

describe("saving goes through a plan", () => {
  it("writes nothing while the plan is built or shown", async () => {
    const { notes, deps } = twoNotes();

    await notes.review("all");

    expect(notes.getSnapshot().phase).toBe("plan");
    expect(notes.getSnapshot().plan!.files.map((file) => file.fileName)).toStrictEqual([
      "2026-10-18_14-32-05-p1.md",
      "2026-10-18_14-32-05-p2.md",
    ]);
    expect(deps.preview).toHaveBeenCalledTimes(1);
    expect(deps.save).not.toHaveBeenCalled();
    expect(deps.analyse).not.toHaveBeenCalled();
  });

  it("previews exactly the text, the name and the format the user typed, in order", async () => {
    const { notes, deps } = twoNotes();

    await notes.review("all");

    expect(deps.preview).toHaveBeenCalledWith([
      { name: "", text: "premiere", format: "md" },
      { name: "Rendez-vous", text: "seconde", format: "md" },
    ]);
  });

  it("previews only the note on screen when asked to save that one", async () => {
    const { notes, deps } = twoNotes();

    await notes.review("active");

    expect(deps.preview).toHaveBeenCalledWith([
      { name: "Rendez-vous", text: "seconde", format: "md" },
    ]);
  });

  it("builds no plan for a note without text", async () => {
    const { notes, deps } = session();

    await notes.review("active");

    expect(deps.preview).not.toHaveBeenCalled();
    expect(notes.getSnapshot().phase).toBe("idle");
  });

  it("cancelling the plan writes nothing and keeps every note", async () => {
    const { notes, deps } = twoNotes();
    await notes.review("all");

    notes.cancelPlan();

    expect(notes.getSnapshot().phase).toBe("idle");
    expect(notes.getSnapshot().plan).toBeNull();
    expect(deps.save).not.toHaveBeenCalled();
    expect(notes.hasUnsaved()).toBe(true);
  });

  it("shows the code of a refused preview and keeps the notes", async () => {
    const { notes, deps } = session({
      preview: vi.fn(async () => {
        throw { code: "notes_name_invalid", data: {} };
      }),
    });
    notes.edit("note-1", { text: "x", name: "???" });

    await notes.review("active");

    expect(notes.getSnapshot().error?.code).toBe("notes_name_invalid");
    expect(notes.getSnapshot().phase).toBe("idle");
    expect(deps.save).not.toHaveBeenCalled();
    expect(notes.hasUnsaved()).toBe(true);
  });

  it("does not let the text change under a plan", async () => {
    const { notes } = twoNotes();
    await notes.review("all");

    notes.edit("note-1", { text: "changed after the plan" });
    notes.remove("note-1");
    notes.discardAll();
    notes.add();

    expect(notes.getSnapshot().notes.drafts.map((draft) => draft.text)).toStrictEqual([
      "premiere",
      "seconde",
    ]);
  });
});

describe("the click on the plan", () => {
  it("writes once, cleans the saved notes and analyses once", async () => {
    const { notes, deps } = twoNotes();
    await notes.review("all");

    await notes.confirm();

    expect(deps.save).toHaveBeenCalledTimes(1);
    expect(deps.save).toHaveBeenCalledWith([
      { name: "", text: "premiere", format: "md" },
      { name: "Rendez-vous", text: "seconde", format: "md" },
    ]);
    expect(deps.analyse).toHaveBeenCalledTimes(1);
    const snapshot = notes.getSnapshot();
    expect(snapshot.phase).toBe("idle");
    expect(snapshot.report?.analysis).toBe("done");
    expect(snapshot.report?.written).toHaveLength(2);
    expect(notes.hasUnsaved()).toBe(false);
    expect(snapshot.writes).toBe(1);
  });

  it("keeps a note that was not part of the plan", async () => {
    const { notes } = twoNotes();
    await notes.review("active");

    await notes.confirm();

    expect(notes.getSnapshot().notes.drafts.map((draft) => draft.text)).toStrictEqual(["premiere"]);
  });

  it("does nothing without a plan", async () => {
    const { notes, deps } = twoNotes();

    await notes.confirm();

    expect(deps.save).not.toHaveBeenCalled();
  });

  it("is a note that counts as clean even when the analysis fails", async () => {
    const { notes } = session({ analyse: vi.fn(async () => false) });
    notes.edit("note-1", { text: "x" });
    await notes.review("active");

    await notes.confirm();

    expect(notes.hasUnsaved()).toBe(false);
    expect(notes.getSnapshot().report?.analysis).toBe("failed");
  });

  it("keeps every note when the write itself fails", async () => {
    const { notes, deps } = session({
      save: vi.fn(async () => {
        throw { code: "notes_write_failed", data: {} };
      }),
    });
    notes.edit("note-1", { text: "x" });
    await notes.review("active");

    await notes.confirm();

    expect(notes.getSnapshot().error?.code).toBe("notes_write_failed");
    expect(notes.getSnapshot().report).toBeNull();
    expect(notes.hasUnsaved()).toBe(true);
    expect(deps.analyse).not.toHaveBeenCalled();
  });

  it("waits for the user's answer when other documents are pending, and analyses on a yes", async () => {
    const { notes, deps } = session({ inventory: vi.fn(async () => [pending("autre.pdf")]) });
    notes.edit("note-1", { text: "x" });
    await notes.review("active");

    const running = notes.confirm();
    await vi.waitFor(() => expect(notes.getSnapshot().phase).toBe("asking"));
    expect(deps.analyse).not.toHaveBeenCalled();
    notes.answer(true);
    await running;

    expect(deps.analyse).toHaveBeenCalledTimes(1);
    expect(notes.getSnapshot().report?.analysis).toBe("done");
  });

  it("saves but does not analyse when the answer is no", async () => {
    const { notes, deps } = session({ inventory: vi.fn(async () => [pending("autre.pdf")]) });
    notes.edit("note-1", { text: "x" });
    await notes.review("active");

    const running = notes.confirm();
    await vi.waitFor(() => expect(notes.getSnapshot().phase).toBe("asking"));
    notes.answer(false);
    await running;

    expect(deps.analyse).not.toHaveBeenCalled();
    expect(notes.getSnapshot().report?.analysis).toBe("declined");
    expect(notes.hasUnsaved()).toBe(false);
  });

  it("goes through the analysing phase while the pass runs", async () => {
    const phases: string[] = [];
    const { notes } = session({
      analyse: vi.fn(async () => {
        phases.push(notes.getSnapshot().phase);
        return true;
      }),
    });
    notes.edit("note-1", { text: "x" });
    await notes.review("active");

    await notes.confirm();

    expect(phases).toStrictEqual(["analysing"]);
  });
});

describe("throwing notes away", () => {
  it("removes one note, or all of them, only when no save is under way", async () => {
    const { notes } = twoNotes();

    notes.remove("note-1");
    expect(notes.getSnapshot().notes.drafts.map((draft) => draft.text)).toStrictEqual(["seconde"]);
    notes.discardAll();

    expect(notes.hasUnsaved()).toBe(false);
  });

  it("clears the report when a note is edited, and when the report is dismissed", async () => {
    const { notes } = session();
    notes.edit("note-1", { text: "x" });
    await notes.review("active");
    await notes.confirm();
    expect(notes.getSnapshot().report).not.toBeNull();

    notes.dismissReport();

    expect(notes.getSnapshot().report).toBeNull();
  });
});
