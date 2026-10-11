import type { Translator } from "../i18n/translate";
import type { Suggestion, SuggestionSet } from "./ipc";

/**
 * Question suggestions, written for the screen.
 *
 * Rust sends machine data (a template id and the slots, `knowledge::suggest`); every sentence is
 * written here from the catalogue, in the user's language, exactly as the tabular nudge writes its
 * example. A suggestion is only ever a question the user can send as it stands: Rust has already
 * proved, on her own files, that it has an answer, and `tests/knowledge_surfaces.rs` proves that
 * each catalogue sentence is read the way the data says (a classifier and a router answer it).
 *
 * Nothing here is stored. A suggestion is computed when the screen is drawn.
 */

/** The month's name in the language of the interface: "mars" and "March". */
export function monthName(locale: string, month: number): string {
  const date = new Date(Date.UTC(2026, month - 1, 1));
  return new Intl.DateTimeFormat(locale, { month: "long", timeZone: "UTC" }).format(date);
}

/** Templates whose sentence has a variant that names the workbook. */
const HAS_FILE_VARIANT = new Set([
  "entity_rows_count",
  "entity_total",
  "entity_period_count",
  "top_group",
]);

/** The catalogue key of a suggestion's sentence. The variant that names a workbook is used when
 * the suggestion carries one: without it the product would ask which workbook was meant. */
export function suggestionKey(suggestion: Suggestion): string {
  const base = `suggestions.${suggestion.templateId}`;
  return suggestion.file !== null && HAS_FILE_VARIANT.has(suggestion.templateId)
    ? `${base}InFile`
    : base;
}

/** The sentence to show and to send. */
export function suggestionText(t: Translator, locale: string, suggestion: Suggestion): string {
  const values: Record<string, string> = {};
  if (suggestion.entity !== null) {
    values.name = suggestion.entity.display;
  }
  if (suggestion.value !== null) {
    values.value = suggestion.value;
  }
  if (suggestion.column !== null) {
    values.column = suggestion.column;
  }
  if (suggestion.group !== null) {
    values.group = suggestion.group;
  }
  if (suggestion.file !== null) {
    values.file = suggestion.file;
  }
  if (suggestion.month !== null) {
    values.month = monthName(locale, suggestion.month);
  }
  if (suggestion.year !== null) {
    values.year = String(suggestion.year);
  }
  return t(suggestionKey(suggestion), values);
}

/** Whether a suggestion puts somebody's name on the screen. */
export function namesSomebody(suggestion: Suggestion): boolean {
  return suggestion.entity !== null;
}

/** What is drawn for a set: the sentences, and whether to offer the action that shows the names. */
export interface SuggestionView {
  items: { key: string; text: string; suggestion: Suggestion }[];
  /** Offer "show suggestions with names". */
  canRevealNames: boolean;
  /** Offer "hide the names again": names are on screen because of the user's own click. */
  canHideNames: boolean;
}

export function suggestionView(
  t: Translator,
  locale: string,
  set: SuggestionSet | null,
  revealed: boolean,
): SuggestionView {
  if (set === null) {
    return { items: [], canRevealNames: false, canHideNames: false };
  }
  const items = set.suggestions.map((suggestion, index) => ({
    key: `${index}-${suggestion.templateId}-${suggestion.entity?.entityId ?? ""}`,
    text: suggestionText(t, locale, suggestion),
    suggestion,
  }));
  return {
    items,
    canRevealNames: !revealed && set.namesHidden,
    canHideNames: revealed && set.suggestions.some(namesSomebody),
  };
}
