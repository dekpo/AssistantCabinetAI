import type { Translator } from "../i18n/translate";
import type { FolderAnswer, TabularAnswer } from "./ipc";

/**
 * A "which one?" answer turned into buttons (HAP-1, UX-1; `docs/test-reports/human-acceptance-pass-1/05-
 * analysis-notes.md`, section 5). The product asks which file she means and, until now, expected her to
 * retype the whole question with the name. A choice replaces her question in place, through the
 * existing "edit and resend", so the conversation reads as if she had been specific from the start and
 * the clarification, which carries nothing worth remembering, leaves the memory.
 */
export interface Choice {
  /** The path shown on the button and written into the question. */
  path: string;
  /** What she wrote that matched several files ("neurologie.pdf"), replaced by the path; `null` for
   * "which workbook", where nothing in her question named one. */
  query: string | null;
}

export function choicesFromFolder(answer: FolderAnswer): Choice[] {
  return answer.kind === "ambiguous_reference"
    ? answer.candidates.map((file) => ({ path: file.relativePath, query: answer.query }))
    : [];
}

export function choicesFromTabular(answer: TabularAnswer): Choice[] {
  switch (answer.kind) {
    case "which_workbook":
      return answer.candidates.map((path) => ({ path, query: null }));
    case "ambiguous_reference":
      return answer.candidates.map((path) => ({ path, query: answer.query }));
    default:
      return [];
  }
}

/**
 * Her question with the chosen file written into it: the ambiguous name replaced by the full path
 * when it occurs in her words (any case), otherwise the path appended in her language. Never
 * anything that was not hers or the file's own path.
 */
export function rewriteQuestion(t: Translator, question: string, choice: Choice): string {
  const base = question.trim();
  if (choice.query !== null && choice.query.length > 0) {
    const at = base.toLowerCase().indexOf(choice.query.toLowerCase());
    if (at >= 0) {
      return base.slice(0, at) + choice.path + base.slice(at + choice.query.length);
    }
    return base + t("choices.appendFile", { path: choice.path });
  }
  return base + t("choices.appendWorkbook", { path: choice.path });
}
