import type {
  CalculationExplanation,
  CalculationFollowUp,
  CombinedScope,
  ShownCalculation,
  TableCarryOver,
  TabularAnswer,
} from "./ipc";
import type { Translator } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";

/**
 * What the user can ask under a computed figure (TABULAR step 06, owner decision 11 of 20-21 October
 * 2026): "Expliquer le calcul" explains the method and leaves the figure alone; "Demander a l'IA"
 * analyses the figure. Everything here is pure, so the rules are tested without a window.
 */

/** The figure an answer carries, with the selection it was given under. The interface keeps it so that
 * a later press asks about the figure on screen, not about whatever is ticked by then. */
export interface StoredCalculation {
  shown: ShownCalculation;
  /** The selection the answer was given under (`undefined`: the default scope of the conversation). */
  scope: CombinedScope | undefined;
  /** This answer is already the analysis of the figure: "Regenerer" asks for the analysis again. */
  analysed?: true;
  /** The state the question of this answer was asked with, kept only when it was applied (a follow-up
   * such as "et pour mars ?"): the short question gives no figure without it, so the explanation and
   * the analysis send it back (TABULAR step 11). */
  tableCarryOver?: TableCarryOver;
}

/** The explanation block under a figure. */
export interface EntryExplanation {
  text: string;
  status: "running" | "done" | "unavailable";
  unverifiedNumbers: string[];
  modelArithmetic: string[];
}

/** The figure of a tabular answer, as the request wants it back; nothing for any answer that is not a
 * computed value (a nudge, a structural fact, a human choice have no figure to explain). */
export function storedCalculationOf(
  answer: TabularAnswer | null | undefined,
  scope: CombinedScope | undefined,
  analysed = false,
  tableCarryOver?: TableCarryOver,
): StoredCalculation | undefined {
  if (answer === null || answer === undefined || answer.kind !== "value") {
    return undefined;
  }
  return {
    shown: { file: answer.file, value: answer.value, derivation: answer.derivation },
    scope,
    ...(analysed ? { analysed: true as const } : {}),
    ...(tableCarryOver === undefined ? {} : { tableCarryOver }),
  };
}

/** The request parameter for one of the two actions. */
export function followUpOf(
  kind: CalculationFollowUp["kind"],
  calculation: StoredCalculation,
): CalculationFollowUp {
  return {
    kind,
    shown: calculation.shown,
    ...(calculation.tableCarryOver === undefined ? {} : { tableCarryOver: calculation.tableCarryOver }),
  };
}

/** A new explanation block: empty and running. Pressing the button again starts from this, so the
 * block is replaced and never stacked on the one before. */
export function beginExplanation(): EntryExplanation {
  return { text: "", status: "running", unverifiedNumbers: [], modelArithmetic: [] };
}

/** One more piece of the explanation as it is written. A model's chat template often opens with a
 * space token: it is dropped from the very first piece only. */
export function appendExplanation(
  current: EntryExplanation | undefined,
  delta: string,
): EntryExplanation {
  const base = current ?? beginExplanation();
  const piece = base.text.length === 0 ? delta.replace(/^ /, "") : delta;
  return { ...base, text: base.text + piece };
}

/** The explanation once the call has ended: the checked text, or the sentence that it could not be
 * written (the figure stays either way). */
export function settleExplanation(result: CalculationExplanation): EntryExplanation {
  return {
    text: result.text,
    status: result.unavailable ? "unavailable" : "done",
    unverifiedNumbers: result.unverifiedNumbers,
    modelArithmetic: result.modelArithmetic,
  };
}

/** The entry with its explanation replaced by `explanation`, or without one when that is `undefined`:
 * the block of an earlier press is never kept beside the new one. */
export function withExplanation(
  entry: ChatEntry,
  explanation: EntryExplanation | undefined,
): ChatEntry {
  const { explanation: previous, ...rest } = entry;
  void previous;
  return explanation === undefined ? rest : { ...rest, explanation };
}

/** The explanation when she stopped it: what was written stays, marked done; nothing written, no block. */
export function stoppedExplanation(current: EntryExplanation | undefined): EntryExplanation | undefined {
  if (current === undefined || current.text.trim().length === 0) {
    return undefined;
  }
  return { ...current, status: "done" };
}

/** The title and text of the explanation, for the copy and the read-aloud. `null` while it runs or when
 * there is nothing to say. */
export function explanationBlock(t: Translator, entry: ChatEntry): string | null {
  const explanation = entry.explanation;
  if (explanation === undefined || explanation.status === "running") {
    return null;
  }
  const body =
    explanation.status === "unavailable" ? t("chat.explanationUnavailable") : explanation.text;
  if (body.trim().length === 0) {
    return null;
  }
  return [t("chat.explanationTitle"), "", body].join("\n");
}

/** The buttons under an answer: which of the two new actions, and the label of the one that asks again. */
export interface MessageButtons {
  explain: boolean;
  analyse: boolean;
  regenerate: "actions.regenerate" | "actions.regenerateWithModel" | null;
}

type ButtonFlags = Pick<
  ChatEntry,
  | "tabular"
  | "deterministic"
  | "tabularRetryable"
  | "documentsNotNeeded"
  | "mixed"
  | "mixedTable"
  | "mixedDocumentsUnavailable"
  | "tabularNudge"
  | "calculation"
>;

/**
 * The buttons of one finished answer, from the flags it carries.
 *
 * - A computed value in tables only, and the data-only answer of door 3 (`documentsNotNeeded`): two
 *   buttons, "Expliquer le calcul" and "Demander a l'IA" (the analysis). Nothing asks the model to read
 *   the question again: that meaning is gone for a figure.
 * - A nudge (no figure): one button, "Demander a l'IA", with its old meaning (the model tries to
 *   understand the question).
 * - A structural fact and a human choice (which column, which workbook...): none, as before.
 * - A mixed answer whose table part is a computed value, written by the model: "Regenerer" and
 *   "Expliquer le calcul", no analysis (it would be a second "Regenerer"). When the model wrote nothing
 *   and the figure stands alone, it is the two-button case.
 * - A tables-only advisory answer (a mixed answer with no table part and no document selected): the
 *   one button "Regenerer", which asks the writer again.
 * - Any other answer: "Regenerer", or "Demander a l'IA" for an answer the work folder computed.
 */
export function buttonsFor(entry: ButtonFlags): MessageButtons {
  const figure = entry.calculation !== undefined;
  if (entry.tabular === true) {
    if (figure) {
      return { explain: true, analyse: true, regenerate: null };
    }
    if (entry.tabularRetryable !== true) {
      return { explain: false, analyse: false, regenerate: null };
    }
    return { explain: false, analyse: false, regenerate: "actions.regenerateWithModel" };
  }
  if (entry.mixed === true && entry.mixedTable?.kind === "value" && figure) {
    return entry.deterministic === true
      ? { explain: true, analyse: true, regenerate: null }
      : { explain: true, analyse: false, regenerate: "actions.regenerate" };
  }
  return {
    explain: false,
    analyse: false,
    regenerate: entry.deterministic === true ? "actions.regenerateWithModel" : "actions.regenerate",
  };
}
