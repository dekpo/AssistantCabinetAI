import type { EntityChoice, OutsideSelection } from "./ipc";

/**
 * The `@` picker of the composer: typing `@` and the first letters of a name lists the names of the
 * selection, and choosing one puts a chip above the box. The pure parts live here so they can be
 * tested without a window; `EntityPicker.tsx` and `Composer.tsx` only draw and forward keys.
 */

/** Names listed at once. */
export const PICKER_LIMIT = 8;

/** Wait after the last keystroke before asking Rust, so typing a name is one search, not six. */
export const PICKER_DEBOUNCE_MS = 120;

/** A name chosen for the next question. It is an id and the words shown; the question text itself
 * never changes (the model reads plain text). */
export interface Chip {
  entityId: number;
  display: string;
}

/** The `@word` the caret is inside: where it starts and the letters typed after the `@`. */
export interface Mention {
  start: number;
  prefix: string;
}

/**
 * The mention the caret sits in, or `null`. An `@` counts only at the start of the text or after
 * whitespace (an e-mail address is not a mention), and the mention ends at the first whitespace.
 */
export function mentionAt(text: string, caret: number): Mention | null {
  const end = Math.min(Math.max(caret, 0), text.length);
  let start = end;
  while (start > 0 && !/\s/u.test(text[start - 1] ?? "")) {
    start -= 1;
  }
  // A caret before the `@` (or on a bare word) is not inside a mention.
  if (text[start] !== "@" || end === start) {
    return null;
  }
  return { start, prefix: text.slice(start + 1, end) };
}

/** The text with the mention replaced by the name, and where the caret goes (after the name and
 * one space). */
export function insertMention(
  text: string,
  mention: Mention,
  caret: number,
  display: string,
): { text: string; caret: number } {
  const before = text.slice(0, mention.start);
  const after = text.slice(Math.min(Math.max(caret, mention.start), text.length));
  const inserted = `${display} `;
  // A space already follows the caret: do not stack a second one.
  const rest = after.startsWith(" ") ? after.slice(1) : after;
  return { text: `${before}${inserted}${rest}`, caret: before.length + inserted.length };
}

/** An `@` put at the caret (the button next to the box), with a space before it when the caret
 * follows a word, so it starts a mention instead of ending an e-mail address. The caret lands after
 * the `@`. */
export function insertAt(text: string, caret: number): { text: string; caret: number } {
  const at = Math.min(Math.max(caret, 0), text.length);
  const before = text.slice(0, at);
  const lead = before.length > 0 && !/\s$/u.test(before) ? " " : "";
  const inserted = `${lead}@`;
  return {
    text: `${before}${inserted}${text.slice(at)}`,
    caret: before.length + inserted.length,
  };
}

/** A chip added once: picking the same name twice does nothing. */
export function withChip(chips: readonly Chip[], chip: Chip): Chip[] {
  return chips.some((each) => each.entityId === chip.entityId) ? [...chips] : [...chips, chip];
}

export function withoutChip(chips: readonly Chip[], entityId: number): Chip[] {
  return chips.filter((each) => each.entityId !== entityId);
}

export function chipIds(chips: readonly Chip[]): number[] {
  return chips.map((chip) => chip.entityId);
}

// ---------------------------------------------------------------------------------------------
// Keyboard
// ---------------------------------------------------------------------------------------------

export type PickerAction =
  | { kind: "move"; index: number }
  | { kind: "choose"; index: number }
  | { kind: "close" }
  | { kind: "ignore" };

/**
 * What a key does while the list is open. Arrows wrap around, Enter and Tab choose the highlighted
 * name, Escape closes. With nothing to choose, Enter and Tab are the ordinary keys again, so a
 * question can still be sent while the list shows nothing.
 */
export function pickerKey(key: string, highlighted: number, count: number): PickerAction {
  if (key === "Escape") {
    return { kind: "close" };
  }
  if (count === 0) {
    return { kind: "ignore" };
  }
  if (key === "ArrowDown") {
    return { kind: "move", index: (highlighted + 1) % count };
  }
  if (key === "ArrowUp") {
    return { kind: "move", index: (highlighted - 1 + count) % count };
  }
  if (key === "Enter" || key === "Tab") {
    return { kind: "choose", index: Math.min(Math.max(highlighted, 0), count - 1) };
  }
  return { kind: "ignore" };
}

// ---------------------------------------------------------------------------------------------
// Search results
// ---------------------------------------------------------------------------------------------

/** Names that are not already a chip. */
export function offeredChoices(
  choices: readonly EntityChoice[],
  chips: readonly Chip[],
): EntityChoice[] {
  const taken = new Set(chipIds(chips));
  return choices.filter((choice) => !taken.has(choice.entityId));
}

/** Every file path an outside-selection name lives in, documents first. */
export function outsidePaths(outside: OutsideSelection): { documents: string[]; workbooks: string[] } {
  return { documents: [...outside.documents], workbooks: [...outside.workbooks] };
}

export function outsideFileCount(outside: OutsideSelection): number {
  return outside.documents.length + outside.workbooks.length;
}

// ---------------------------------------------------------------------------------------------
// Debounce
// ---------------------------------------------------------------------------------------------

export interface Debounced<Args extends unknown[]> {
  (...args: Args): void;
  cancel: () => void;
}

/** `call` runs once, `wait` milliseconds after the last invocation. */
export function debounce<Args extends unknown[]>(
  call: (...args: Args) => void,
  wait: number,
): Debounced<Args> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const wrapped = ((...args: Args) => {
    if (timer !== undefined) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = undefined;
      call(...args);
    }, wait);
  }) as Debounced<Args>;
  wrapped.cancel = () => {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
  };
  return wrapped;
}
