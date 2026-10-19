import { readFileSync } from "node:fs";
import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../../i18n/I18nProvider";
import type { CentreSection } from "../../lib/centreSection";
import { createNotesSession, type NotesSnapshot } from "../../lib/notesSession";
import type { NoteFile } from "../../speech/ipc";
import { ConfirmDialog } from "../ConfirmDialog";
import { TitleBar } from "../TitleBar";
import { WorkspacePanel } from "../WorkspacePanel";
import { createViewStateStore } from "../../lib/viewState";
import { CentreSwitch } from "./CentreSwitch";
import { AnalysisChoiceDialog, RecordingDialog, UnsavedNoteDialog } from "./NotesDialogs";
import { DictationPill } from "./DictationPill";
import type { NotesDictation } from "./LiveDictation";
import { NotesPanel, type NotesActions, type NotesBundle } from "./NotesPanel";

/* Drawn to static markup with the real catalogues, as in `render.test.ts`: nothing is clicked, so
   what is proven here is what is on screen in each state of the Notes section. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(element: ReactElement, locale = "en-US"): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

const ACTIONS: NotesActions = {
  select: () => {},
  add: () => {},
  edit: () => {},
  discard: () => {},
  review: () => {},
  confirm: () => {},
  cancelPlan: () => {},
  dismissReport: () => {},
  reveal: async () => {},
};

function file(name: string): NoteFile {
  return {
    relativePath: `Notes/${name}`,
    fileName: name,
    bytes: 3,
    chars: 3,
    lines: 1,
  };
}

function snapshotOf(texts: string[], change: Partial<NotesSnapshot> = {}): NotesSnapshot {
  const session = createNotesSession({
    preview: async () => [],
    save: async () => [],
    inventory: async () => [],
    analyse: async () => true,
  });
  texts.forEach((text, index) => {
    if (index > 0) {
      session.add();
    }
    session.edit(`note-${index + 1}`, { text });
  });
  return { ...session.getSnapshot(), ...change };
}

function panel(snapshot: NotesSnapshot, extra: Partial<NotesBundle> = {}): string {
  return page(
    createElement(NotesPanel, {
      bundle: { snapshot, actions: ACTIONS, hasFolder: true, dictation: null, ...extra },
    }),
  );
}

describe("the Notes section", () => {
  it("shows an empty note with its switcher, and no save block", () => {
    const html = panel(snapshotOf([]));

    expect(html).toContain("notes__text");
    expect(html).toContain("Note 1");
    expect(html).toContain("New note");
    expect(html).toContain("Discard this note");
    expect(html).not.toContain("notes__save");
    expect(html).not.toContain("pill");
  });

  it("offers the Raw and Preview switch for Markdown", () => {
    const html = panel(snapshotOf(["# Title"]));

    expect(html).toContain("viewer__modes");
    expect(html).toContain("Raw");
    expect(html).toContain("Preview");
  });

  it("shows the save block once there is text: name with the time as hint, format, Save", () => {
    const html = panel(snapshotOf(["Hello"]));

    expect(html).toContain("notes__save");
    expect(html).toMatch(/placeholder="\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}"/);
    expect(html).toContain("Markdown (.md)");
    expect(html).toContain("Plain text (.txt)");
    expect(html).toContain("Save this note");
    expect(html).not.toContain("Save all notes");
  });

  it("offers to save all of them when several notes have text", () => {
    const html = panel(snapshotOf(["one", "two"]));

    expect(html).toContain("Save all notes (2)");
    expect(html).toContain("Note 1");
    expect(html).toContain("Note 2");
  });

  it("names a note in the switcher by what the user typed for its file name", () => {
    const base = snapshotOf(["one"]);
    const named = {
      ...base,
      notes: {
        ...base.notes,
        drafts: base.notes.drafts.map((draft) => ({ ...draft, name: "Visit" })),
      },
    };

    expect(panel(named)).toContain(">Visit<");
  });

  it("disables Save and says why when no documents folder is chosen", () => {
    const html = panel(snapshotOf(["Hello"]), { hasFolder: false });

    expect(html).toContain("Notes need a documents folder");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Save this note/);
  });

  it("draws the plan with the exact files, and writes nothing yet", () => {
    const html = panel(
      snapshotOf(["Hello"], {
        phase: "plan",
        plan: {
          drafts: [{ id: "note-1", text: "Hello" }],
          files: [file("2026-10-18_14-32-05-visit.md"), file("2026-10-18_14-32-05-b.md")],
        },
      }),
    );

    expect(html).toContain("Files that will be written");
    expect(html).toContain("Notes/2026-10-18_14-32-05-visit.md");
    expect(html).toContain("Write the 2 files");
    expect(html).toContain("Nothing is written until you confirm");
  });

  it("locks the text and the buttons while a save is under way, and says what is happening", () => {
    const html = panel(snapshotOf(["Hello"], { phase: "analysing" }));

    expect(html).toMatch(/<textarea[^>]*readOnly=""/);
    expect(html).toContain("Analysing. The Documents panel shows the progress.");
    expect(html).toContain("spinner");
  });

  it("reports the written paths with Show in folder, and the analysis status", () => {
    const html = panel(
      snapshotOf([], {
        report: { written: [file("2026-10-18_14-32-05.md")], analysis: "done" },
      }),
    );

    expect(html).toContain("Note saved");
    expect(html).toContain("Notes/2026-10-18_14-32-05.md");
    expect(html).toContain("Show in folder");
    expect(html).toContain("The note is analysed");
  });

  it("says a declined analysis is still to do, and a failed one with its own sentence", () => {
    const declined = panel(
      snapshotOf([], { report: { written: [file("a.md")], analysis: "declined" } }),
    );
    const failed = panel(
      snapshotOf([], { report: { written: [file("a.md")], analysis: "failed" } }),
    );

    expect(declined).toContain("saved but not analysed yet");
    expect(failed).toContain("The note was saved but could not be analysed");
  });

  it("shows the code of a refused save in words", () => {
    const html = panel(
      snapshotOf(["Hello"], { error: { code: "notes_name_invalid", data: {} } }),
    );

    expect(html).toContain("This note name cannot be used");
  });

  it("is French in French", () => {
    const html = page(
      createElement(NotesPanel, {
        bundle: { snapshot: snapshotOf(["Bonjour"]), actions: ACTIONS, hasFolder: true, dictation: null },
      }),
      "fr-FR",
    );

    expect(html).toContain("Nouvelle note");
    expect(html).toContain("Enregistrer cette note");
    expect(html).toContain("Brut");
  });
});

const LIVE: NotesDictation = {
  phase: "idle",
  recording: false,
  disabled: false,
  reasonKey: null,
  level: { get: () => 0, subscribe: () => () => {} },
  startedAt: null,
  waiting: 0,
  failed: [],
  error: null,
  capReached: false,
  onToggle: () => {},
  onCancel: () => {},
  onDismiss: () => {},
};

describe("the dictation pill", () => {
  const dictation = { recording: false, level: 0, elapsedMs: 0, onToggle: () => {} };

  it("is drawn only when dictation is handed to the panel", () => {
    expect(panel(snapshotOf([]), { dictation: LIVE })).toContain("pill__button");
    expect(panel(snapshotOf([]), { dictation: null })).not.toContain("pill");
  });

  it("shows a microphone at rest and no hint", () => {
    const html = page(createElement(DictationPill, dictation));

    expect(html).toContain('aria-label="Dictate"');
    expect(html).toContain("<svg");
    expect(html).not.toContain("pill__bars");
    expect(html).not.toContain("Click again to finish");
  });

  it("shows the waveform, the elapsed time and the hint while recording", () => {
    const html = page(
      createElement(DictationPill, { ...dictation, recording: true, level: 1, elapsedMs: 65_000 }),
    );

    expect(html).toContain("pill__bars");
    expect(html.match(/pill__bar"/g)).toHaveLength(7);
    expect(html).toContain("1:05");
    expect(html).toContain("Click again to finish");
    expect(html).toContain('aria-pressed="true"');
  });
});

describe("the Files | Notes switch", () => {
  it("is a two-position tab list with the chosen section selected", () => {
    const html = page(createElement(CentreSwitch, { section: "notes", onChange: () => {} }));

    expect(html).toContain('role="tablist"');
    expect(html).toMatch(/id="centre-tab-files"[^>]*aria-selected="false"/);
    expect(html).toMatch(/id="centre-tab-notes"[^>]*aria-selected="true"/);
    expect(html).not.toContain("centre__dot");
  });

  it("carries the red dot on Notes while a recording runs", () => {
    const html = page(
      createElement(CentreSwitch, { section: "files", recording: true, onChange: () => {} }),
    );

    expect(html).toContain("centre__dot");
    expect(html).toContain("Recording in progress");
  });
});

describe("the centre zone", () => {
  const viewStates = createViewStateStore();
  const tabActions = { activate: () => {}, close: () => {}, missing: () => {} };
  const bundle: NotesBundle = {
    snapshot: snapshotOf(["Hello"]),
    actions: ACTIONS,
    hasFolder: true,
    dictation: null,
  };

  function centre(notes: { section: CentreSection } | undefined): string {
    return page(
      createElement(WorkspacePanel, {
        view: { kind: "welcome" },
        viewStates,
        actions: tabActions,
        notes:
          notes === undefined
            ? undefined
            : { section: notes.section, onSection: () => {}, recording: false, bundle },
      }),
    );
  }

  it("draws exactly the files' content when the Notes switch is off", () => {
    const html = centre(undefined);

    expect(html.startsWith('<section class="workspace">')).toBe(true);
    expect(html).not.toContain("centre");
    expect(html).not.toContain("notes");
  });

  it("keeps both sections mounted and hides the one that is not shown", () => {
    const onNotes = centre({ section: "notes" });
    const onFiles = centre({ section: "files" });

    expect(onNotes).toMatch(/id="centre-panel-files"[^>]*hidden=""/);
    expect(onNotes).not.toMatch(/id="centre-panel-notes"[^>]*hidden=""/);
    expect(onFiles).toMatch(/id="centre-panel-notes"[^>]*hidden=""/);
    expect(onFiles).not.toMatch(/id="centre-panel-files"[^>]*hidden=""/);
    for (const html of [onNotes, onFiles]) {
      expect(html).toContain("Workspace");
      expect(html).toContain("notes__text");
    }
  });
});

describe("the question before a note can be lost", () => {
  it("has three answers, in the order Cancel, Discard, Review and save", () => {
    const html = page(
      createElement(UnsavedNoteDialog, {
        count: 1,
        onReview: () => {},
        onDiscard: () => {},
        onCancel: () => {},
      }),
    );

    expect(html).toContain("A note has not been saved.\nIf you go on, it is lost.");
    const order = ["Cancel", "Discard", "Review and save"].map((label) => html.indexOf(`>${label}<`));
    expect(order.every((index) => index > 0)).toBe(true);
    expect(order).toStrictEqual([...order].sort((a, b) => a - b));
    expect(html).toContain("button--destructive");
  });

  it("counts the notes when there are several", () => {
    const html = page(
      createElement(UnsavedNoteDialog, {
        count: 3,
        onReview: () => {},
        onDiscard: () => {},
        onCancel: () => {},
      }),
    );

    expect(html).toContain("3 notes have not been saved.\nIf you go on, they are lost.");
  });

  it("asks about the other pending documents in plain words", () => {
    const html = page(
      createElement(AnalysisChoiceDialog, { onAnalyse: () => {}, onSkip: () => {} }),
    );

    expect(html).toContain("Analyze the other documents too?");
    expect(html).toContain("Analyze now");
  });
});

describe("the confirmation dialog the rest of the application uses", () => {
  const props = { title: "T", body: "B", confirmLabel: "OK", onConfirm: () => {}, onCancel: () => {} };

  it("draws byte for byte what it drew before the third answer existed", () => {
    expect(page(createElement(ConfirmDialog, props))).toBe(
      '<div class="confirm__backdrop"><div class="confirm" role="alertdialog" aria-modal="true" aria-label="T"><h2 class="confirm__title">T</h2><p class="confirm__body">B</p><div class="confirm__actions"><button type="button" class="button button--compact">Cancel</button><button type="button" class="button button--compact button--primary">OK</button></div></div></div>',
    );
  });

  it("draws the third answer between the other two, and nowhere else", () => {
    const base = page(createElement(ConfirmDialog, props));
    const three = page(
      createElement(ConfirmDialog, {
        ...props,
        secondary: { label: "Drop", onSelect: () => {}, destructive: true },
      }),
    );

    expect(three).toBe(
      base.replace(
        '<button type="button" class="button button--compact button--primary">',
        '<button type="button" class="button button--compact button--destructive">Drop</button><button type="button" class="button button--compact button--primary">',
      ),
    );
  });
});

describe("the title bar", () => {
  const props = {
    connection: "online" as const,
    onRefresh: () => {},
    onOpenSettings: () => {},
    workspaceOpen: true,
    onToggleWorkspace: () => {},
  };

  it("changes only by the Notes button, between the workspace toggle and Settings", () => {
    const without = page(createElement(TitleBar, props));
    const withNotes = page(
      createElement(TitleBar, { ...props, notes: { open: false, onToggle: () => {} } }),
    );

    const button =
      '<button type="button" class="button button--icon" aria-pressed="false" aria-label="Notes" title="Notes">';
    expect(withNotes).toContain(button);
    const start = withNotes.indexOf(button);
    const end = withNotes.indexOf("</button>", start) + "</button>".length;
    expect(withNotes.slice(0, start) + withNotes.slice(end)).toBe(without);
    expect(start).toBeGreaterThan(withNotes.indexOf('aria-label="Workspace"'));
    expect(start).toBeLessThan(withNotes.indexOf("Settings"));
  });

  it("presses the Notes button while the section is open", () => {
    const html = page(
      createElement(TitleBar, { ...props, notes: { open: true, onToggle: () => {} } }),
    );

    expect(html).toContain('aria-pressed="true" aria-label="Notes"');
  });
});

describe("the line under the note while dictating", () => {
  it("says nothing at rest", () => {
    const html = panel(snapshotOf(["Bonjour"]), { dictation: LIVE });

    expect(html).not.toContain("notes__dictation\"><p");
    expect(html).not.toContain("Discard the recording");
  });

  it("shows the waveform while recording, and the way out", () => {
    const html = panel(snapshotOf(["Bonjour"]), {
      dictation: { ...LIVE, phase: "recording", recording: true, startedAt: 1 },
    });

    expect(html).toContain("pill__bars");
    expect(html).toContain("Discard the recording");
    expect(html).not.toContain("waiting to be written");
  });

  it("counts the passages waiting, in the singular and the plural", () => {
    const one = panel(snapshotOf(["Bonjour"]), {
      dictation: { ...LIVE, phase: "finishing", waiting: 1 },
    });
    const many = panel(snapshotOf(["Bonjour"]), {
      dictation: { ...LIVE, phase: "finishing", waiting: 3 },
    });

    expect(one).toContain("1 passage waiting to be written");
    expect(many).toContain("3 passages waiting to be written");
  });

  it("locks the save and the note buttons while it goes on", () => {
    const html = panel(snapshotOf(["Bonjour"]), {
      dictation: { ...LIVE, phase: "recording", recording: true, startedAt: 1 },
    });
    const save = /<button[^>]*>Save this note<\/button>/.exec(html)?.[0] ?? "";

    expect(save).toContain("disabled");
  });

  it("reports the failed passages by position and the length cap, with a way to close", () => {
    const html = panel(snapshotOf(["Bonjour"]), {
      dictation: { ...LIVE, failed: [2, 5], capReached: true },
    });

    expect(html).toContain("Passages 2, 5 could not be transcribed");
    expect(html).toContain("stopped at the length limit");
    expect(html).toContain("Close");
  });

  it("says why the pill cannot be used", () => {
    const html = panel(snapshotOf([]), {
      dictation: { ...LIVE, disabled: true, reasonKey: "speech.session.microphoneBusy" },
    });

    expect(html).toContain("microphone is already in use");
  });
});

describe("the question before notes are lost while a dictation runs", () => {
  const noop = () => {};

  it("says that the dictation is stopped too, and only then", () => {
    const quiet = page(
      createElement(UnsavedNoteDialog, { count: 2, onReview: noop, onDiscard: noop, onCancel: noop }),
    );
    const live = page(
      createElement(UnsavedNoteDialog, {
        count: 2,
        recording: true,
        onReview: noop,
        onDiscard: noop,
        onCancel: noop,
      }),
    );

    expect(quiet).not.toContain("dictation in progress");
    expect(live).toContain("2 notes have not been saved.");
    expect(live).toContain("The dictation in progress is stopped too.");
  });

  it("is replaced by the recording question when hiding the zone, which keeps the notes", () => {
    const app = readFileSync(new URL("../../App.tsx", import.meta.url), "utf8");

    expect(app).toContain('leaving.exit.kind === "hide-zone" ||');
    expect(app).toContain("recording={dictationActive}");
  });
});

describe("the question before a recording is lost", () => {
  it("offers to keep recording or to stop and discard", () => {
    const html = page(createElement(RecordingDialog, { onStop: () => {}, onCancel: () => {} }));

    expect(html).toContain("Recording in progress");
    expect(html).toContain("Stop and discard");
    expect(html).toContain("Cancel");
  });
});
