import type { Translator } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";
import type { CarriedParts, TableCarryOver } from "./ipc";
import { describeFilters } from "./tabularAnswer";

/**
 * Follow-up questions on a table ("et pour mars ?", "et pour le fournisseur B ?", "et le nombre ?")
 * are understood from a small state the previous table answer left: workbook, sheet, column,
 * operation, filters (TABULAR step 11). The state belongs to the entry of the answer that left it, so
 * what a question is asked with is read off the conversation as it stood just before it: a regenerate
 * or an edit of an earlier question truncates the entries first and therefore reads "the state as it
 * was at that point", with nothing else stored. Rust distrusts it and checks it again.
 */

/** The state to send with the next question: the one the last answer of the conversation left. An
 * answer that left none (a document answer, a nudge, a structural fact) leaves nothing to continue,
 * so a follow-up after it stands alone. */
export function carryOverOf(entries: readonly ChatEntry[]): TableCarryOver | undefined {
  for (let position = entries.length - 1; position >= 0; position -= 1) {
    const entry = entries[position];
    if (entry?.role === "assistant") {
      return entry.tableCarryOver;
    }
  }
  return undefined;
}

/** The line under a follow-up answer: what the question did not say and the previous one supplied.
 * The "Understood as" line of the answer is the whole reading; this one marks what was inherited. */
export function carriedOverLine(t: Translator, locale: string, carried: CarriedParts): string {
  const parts: string[] = [t("tabularAnswer.carriedWorkbook")];
  if (carried.column !== null) {
    parts.push(t("tabularAnswer.carriedColumn", { column: carried.column }));
  }
  if (carried.operation !== null) {
    parts.push(t(`tabularAnswer.carriedOperation.${carried.operation}`));
  }
  if (carried.groupBy !== null) {
    parts.push(t("tabularAnswer.carriedGroupBy", { column: carried.groupBy }));
  }
  if (carried.filters.length > 0) {
    parts.push(describeFilters(t, carried.filters, locale));
  }
  return t("tabularAnswer.carriedOver", { parts: parts.join(t("tabularAnswer.carriedSeparator")) });
}
