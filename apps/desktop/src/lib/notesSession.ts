import type { NoteDraft, NoteFile } from "../speech/ipc";
import { normaliseError, type AppError } from "./errors";
import {
  addDraft,
  discardAll,
  editDraft,
  hasUnsaved,
  initialNotes,
  markWritten,
  removeDrafts,
  selectDraft,
  unsavedDrafts,
  type DraftNote,
  type DraftPatch,
  type NotesState,
} from "./notesDrafts";
import { saveAndAnalyse, type AnalysisOutcome, type SaveDeps } from "./notesFlow";

/**
 * Where the save is. `plan` waits for the user's click on the exact files; `asking` waits for the
 * answer to "analyse the other pending documents too"; nothing is written before `saving`.
 */
export type SavePhase = "idle" | "previewing" | "plan" | "saving" | "asking" | "analysing";

/** The files a save would write, shown before anything is written. */
export interface NotesPlan {
  /** The drafts the plan covers, with the text the preview was made from. */
  drafts: Array<{ id: string; text: string }>;
  files: NoteFile[];
}

/** A save that wrote its files; the analysis is `pending` until the pass has answered. */
export interface NotesReport {
  written: NoteFile[];
  analysis: AnalysisOutcome | "pending";
}

export interface NotesSnapshot {
  notes: NotesState;
  phase: SavePhase;
  plan: NotesPlan | null;
  /** The last finished save, until the user dismisses it or edits a note. */
  report: NotesReport | null;
  error: AppError | null;
  /** Counted up after every write, so the Documents card re-reads its folder at once. */
  writes: number;
}

export interface NotesSessionDeps {
  /** `notes_preview`: the names a save would give. Writes nothing. */
  preview: (drafts: NoteDraft[]) => Promise<NoteFile[]>;
  save: SaveDeps["save"];
  inventory: SaveDeps["inventory"];
  analyse: SaveDeps["analyse"];
}

/** `active` is the note on screen; `all` every note that has text. */
export type SaveScope = "active" | "all";

function toWire(draft: DraftNote): NoteDraft {
  return { name: draft.name, text: draft.text, format: draft.format };
}

/**
 * The notes of the window, held outside React so that a section switch, a hidden centre or a
 * remount of the panel cannot lose them. Memory only. A subscribable store in the shape of
 * `useSyncExternalStore`; the mutating calls are ignored while a save is under way, so a plan
 * always describes the text that is then written.
 */
export function createNotesSession(deps: NotesSessionDeps) {
  let snapshot: NotesSnapshot = {
    notes: initialNotes(),
    phase: "idle",
    plan: null,
    report: null,
    error: null,
    writes: 0,
  };
  const listeners = new Set<() => void>();
  let answerAnalysis: ((accept: boolean) => void) | null = null;

  function set(change: Partial<NotesSnapshot>) {
    snapshot = { ...snapshot, ...change };
    listeners.forEach((listener) => listener());
  }

  const idle = () => snapshot.phase === "idle";

  function edit(id: string, patch: DraftPatch) {
    if (!idle()) {
      return;
    }
    const notes = editDraft(snapshot.notes, id, patch);
    if (notes !== snapshot.notes) {
      set({ notes, report: null, error: null });
    }
  }

  /** The preview of the notes in `scope`. It only reads: the files are written by `confirm`. */
  async function review(scope: SaveScope) {
    if (!idle()) {
      return;
    }
    const chosen =
      scope === "all"
        ? unsavedDrafts(snapshot.notes)
        : unsavedDrafts({
            ...snapshot.notes,
            drafts: snapshot.notes.drafts.filter((draft) => draft.id === snapshot.notes.activeId),
          });
    if (chosen.length === 0) {
      return;
    }
    set({ phase: "previewing", error: null, report: null });
    try {
      const files = await deps.preview(chosen.map(toWire));
      set({
        phase: "plan",
        plan: { drafts: chosen.map(({ id, text }) => ({ id, text })), files },
      });
    } catch (raw: unknown) {
      set({ phase: "idle", plan: null, error: normaliseError(raw) });
    }
  }

  /** The user approved the plan: write exactly the notes it covered, then analyse them. */
  async function confirm() {
    const plan = snapshot.plan;
    if (snapshot.phase !== "plan" || plan === null) {
      return;
    }
    const covered = new Set(plan.drafts.map((entry) => entry.id));
    const drafts = snapshot.notes.drafts.filter((draft) => covered.has(draft.id));
    set({ phase: "saving", plan: null });
    try {
      const report = await saveAndAnalyse(
        {
          save: deps.save,
          inventory: deps.inventory,
          analyse: async () => {
            set({ phase: "analysing" });
            return deps.analyse();
          },
          written: (files) =>
            set({
              notes: markWritten(snapshot.notes, plan.drafts),
              writes: snapshot.writes + 1,
              report: { written: files, analysis: "pending" },
            }),
          confirmAnalysis: () =>
            new Promise<boolean>((resolve) => {
              answerAnalysis = resolve;
              set({ phase: "asking" });
            }),
        },
        drafts.map(toWire),
      );
      set({ phase: "idle", report });
    } catch (raw: unknown) {
      // Only the save itself can throw, before any file exists: the notes stay as they were.
      set({ phase: "idle", error: normaliseError(raw) });
    }
  }

  return {
    getSnapshot: () => snapshot,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    select: (id: string) => set({ notes: selectDraft(snapshot.notes, id) }),
    add: () => {
      if (idle()) {
        set({ notes: addDraft(snapshot.notes) });
      }
    },
    edit,
    /** One note thrown away. The caller has asked first when it had text (`guardExit`). */
    remove: (id: string) => {
      if (idle()) {
        set({ notes: removeDrafts(snapshot.notes, [id]), report: null, error: null });
      }
    },
    /** Every note thrown away: a folder change, the end of the window, Notes switched off. */
    discardAll: () => {
      if (idle()) {
        set({ notes: discardAll(snapshot.notes), report: null, error: null });
      }
    },
    review,
    confirm,
    cancelPlan: () => {
      if (snapshot.phase === "plan") {
        set({ phase: "idle", plan: null });
      }
    },
    /** The answer to "analyse the other pending documents too". */
    answer: (accept: boolean) => {
      const resolve = answerAnalysis;
      answerAnalysis = null;
      resolve?.(accept);
    },
    dismissReport: () => set({ report: null, error: null }),
    hasUnsaved: () => hasUnsaved(snapshot.notes),
  };
}

export type NotesSession = ReturnType<typeof createNotesSession>;
