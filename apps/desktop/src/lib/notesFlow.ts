import type { NoteDraft, NoteFile } from "../speech/ipc";
import type { FileRecord } from "./ipc";
import { needsAnalysisConfirmation } from "./notesAnalysis";

/** What happened to the analysis of the saved notes. The save itself stands whatever it is. */
export type AnalysisOutcome = "done" | "declined" | "failed";

export interface SaveReport {
  written: NoteFile[];
  analysis: AnalysisOutcome;
}

/** The four things a save needs from the rest of the application, injected so the flow is tested
 * without a window. */
export interface SaveDeps {
  /** `notes_save`: writes the reviewed notes, all or none. */
  save: (drafts: NoteDraft[]) => Promise<NoteFile[]>;
  /** A fresh read of the documents folder's files, to count the other pending documents. */
  inventory: () => Promise<FileRecord[]>;
  /** Called once the files exist, before anything else: the notes are clean from this moment. */
  written: (files: NoteFile[]) => void;
  /** Asks the user whether other pending documents may be analysed too; resolves to the answer. */
  confirmAnalysis: () => Promise<boolean>;
  /** The App's Analyse pass (`indexing.run()`); resolves to whether it finished without failing. */
  analyse: () => Promise<boolean>;
}

/**
 * Writes the notes, then analyses them: the click on Save is the user's act (decision D9), and
 * nothing else starts an Analyse. The pass reads the whole folder (the per-file path is not
 * separable), so when other documents are also pending the user is asked first; a folder that
 * cannot be read counts as "ask". A refusal or a failed pass never undoes the save, and an
 * exception only comes from the save itself, before any file exists.
 */
export async function saveAndAnalyse(deps: SaveDeps, drafts: NoteDraft[]): Promise<SaveReport> {
  const written = await deps.save(drafts);
  deps.written(written);

  let ask = true;
  try {
    ask = needsAnalysisConfirmation(
      await deps.inventory(),
      written.map((file) => file.relativePath),
    );
  } catch {
    // The count is unknown: asking is the safe answer.
  }
  if (ask && !(await deps.confirmAnalysis())) {
    return { written, analysis: "declined" };
  }
  let finished = false;
  try {
    finished = await deps.analyse();
  } catch {
    // A pass that throws is a pass that failed; the notes are written either way.
  }
  return { written, analysis: finished ? "done" : "failed" };
}
