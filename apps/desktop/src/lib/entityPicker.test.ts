import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { EntityChoice, OutsideSelection } from "./ipc";
import {
  chipIds,
  debounce,
  insertAt,
  insertMention,
  mentionAt,
  offeredChoices,
  outsideFileCount,
  PICKER_DEBOUNCE_MS,
  PICKER_LIMIT,
  pickerKey,
  withChip,
  withoutChip,
} from "./entityPicker";

describe("finding the @name being typed", () => {
  it("finds a mention at the start and the letters typed after the @", () => {
    expect(mentionAt("@Dup", 4)).toStrictEqual({ start: 0, prefix: "Dup" });
    expect(mentionAt("@", 1)).toStrictEqual({ start: 0, prefix: "" });
  });

  it("finds a mention after a space, in the middle of a question", () => {
    expect(mentionAt("Que dit @Dup du bilan ?", 12)).toStrictEqual({ start: 8, prefix: "Dup" });
  });

  it("reads only up to the caret", () => {
    expect(mentionAt("Que dit @Dupont du bilan ?", 11)).toStrictEqual({ start: 8, prefix: "Du" });
  });

  it("is not fooled by an address, a bare word or a caret after a space", () => {
    expect(mentionAt("ecrire a marie@exemple.org", 26)).toBeNull();
    expect(mentionAt("Que dit Dupont", 14)).toBeNull();
    expect(mentionAt("Que dit @Dupont ", 16)).toBeNull();
  });

  it("keeps accents and hyphens in the prefix", () => {
    expect(mentionAt("@Jean-Pi", 8)?.prefix).toBe("Jean-Pi");
    expect(mentionAt("@Élodie", 7)?.prefix).toBe("Élodie");
  });

  it("copes with a caret outside the text", () => {
    expect(mentionAt("@Dup", 99)).toStrictEqual({ start: 0, prefix: "Dup" });
    expect(mentionAt("@Dup", -3)).toBeNull();
  });
});

describe("choosing a name", () => {
  it("replaces the @word by the name and a space, and puts the caret after them", () => {
    const text = "Que dit @Dup du bilan ?";
    const mention = mentionAt(text, 12);
    expect(mention).not.toBeNull();

    const inserted = insertMention(text, mention!, 12, "Jean Dupont");

    expect(inserted.text).toBe("Que dit Jean Dupont du bilan ?");
    expect(inserted.caret).toBe("Que dit Jean Dupont ".length);
  });

  it("does not stack a second space when one already follows", () => {
    const mention = { start: 0, prefix: "Dup" };

    expect(insertMention("@Dup rappel", mention, 4, "Jean Dupont").text).toBe("Jean Dupont rappel");
  });

  it("works at the very end", () => {
    const inserted = insertMention("Que dit @Dup", { start: 8, prefix: "Dup" }, 12, "Jean Dupont");

    expect(inserted).toStrictEqual({ text: "Que dit Jean Dupont ", caret: 20 });
  });

  it("starts a mention with the button: a space first after a word, none otherwise", () => {
    expect(insertAt("", 0)).toStrictEqual({ text: "@", caret: 1 });
    expect(insertAt("Que dit", 7)).toStrictEqual({ text: "Que dit @", caret: 9 });
    expect(insertAt("Que dit ", 8)).toStrictEqual({ text: "Que dit @", caret: 9 });
    expect(insertAt("Que dit du bilan", 7)).toStrictEqual({
      text: "Que dit @ du bilan",
      caret: 9,
    });
  });
});

describe("the chips above the box", () => {
  const dupont = { entityId: 7, display: "Jean Dupont" };
  const martin = { entityId: 8, display: "Claire Martin" };

  it("adds a name once and removes it by id", () => {
    const chips = withChip(withChip([], dupont), martin);

    expect(withChip(chips, dupont)).toStrictEqual([dupont, martin]);
    expect(withoutChip(chips, 7)).toStrictEqual([martin]);
    expect(chipIds(chips)).toStrictEqual([7, 8]);
  });

  it("does not offer a name that is already a chip", () => {
    const choices = [dupont, martin].map(
      (chip): EntityChoice => ({ ...chip, typeId: "person", documents: 1, workbooks: 0 }),
    );

    expect(offeredChoices(choices, [dupont]).map((choice) => choice.entityId)).toStrictEqual([8]);
  });
});

describe("the keyboard in the list", () => {
  it("moves with the arrows and wraps around", () => {
    expect(pickerKey("ArrowDown", 0, 3)).toStrictEqual({ kind: "move", index: 1 });
    expect(pickerKey("ArrowDown", 2, 3)).toStrictEqual({ kind: "move", index: 0 });
    expect(pickerKey("ArrowUp", 0, 3)).toStrictEqual({ kind: "move", index: 2 });
    expect(pickerKey("ArrowUp", 2, 3)).toStrictEqual({ kind: "move", index: 1 });
  });

  it("chooses the highlighted name with Enter or Tab", () => {
    expect(pickerKey("Enter", 1, 3)).toStrictEqual({ kind: "choose", index: 1 });
    expect(pickerKey("Tab", 2, 3)).toStrictEqual({ kind: "choose", index: 2 });
    // A highlight left past the end by a shorter list still chooses a name that exists.
    expect(pickerKey("Enter", 5, 3)).toStrictEqual({ kind: "choose", index: 2 });
  });

  it("closes with Escape, even when the list is empty", () => {
    expect(pickerKey("Escape", 0, 3)).toStrictEqual({ kind: "close" });
    expect(pickerKey("Escape", 0, 0)).toStrictEqual({ kind: "close" });
  });

  it("leaves Enter and Tab alone when there is nothing to choose, so a question can still be sent", () => {
    expect(pickerKey("Enter", 0, 0)).toStrictEqual({ kind: "ignore" });
    expect(pickerKey("Tab", 0, 0)).toStrictEqual({ kind: "ignore" });
    expect(pickerKey("ArrowDown", 0, 0)).toStrictEqual({ kind: "ignore" });
  });

  it("ignores every other key, so typing goes on", () => {
    expect(pickerKey("a", 0, 3)).toStrictEqual({ kind: "ignore" });
    expect(pickerKey("Backspace", 0, 3)).toStrictEqual({ kind: "ignore" });
  });
});

describe("the names outside the selection", () => {
  it("counts the files of both folders", () => {
    const outside: OutsideSelection = {
      entityId: 1,
      display: "Jean Dupont",
      typeId: "person",
      documents: ["a.txt", "b.txt", "c.txt"],
      workbooks: ["rdv.csv"],
    };

    expect(outsideFileCount(outside)).toBe(4);
  });
});

describe("the wait before a lookup", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("is about a tenth of a second, and the list holds at most eight names", () => {
    expect(PICKER_DEBOUNCE_MS).toBe(120);
    expect(PICKER_LIMIT).toBe(8);
  });

  it("searches once, for the last thing typed", () => {
    const search = vi.fn();
    const lookup = debounce(search, PICKER_DEBOUNCE_MS);

    lookup("D");
    vi.advanceTimersByTime(60);
    lookup("Du");
    vi.advanceTimersByTime(60);
    lookup("Dup");
    expect(search).not.toHaveBeenCalled();
    vi.advanceTimersByTime(PICKER_DEBOUNCE_MS);

    expect(search).toHaveBeenCalledTimes(1);
    expect(search).toHaveBeenCalledWith("Dup");
  });

  it("does not search at all once cancelled", () => {
    const search = vi.fn();
    const lookup = debounce(search, PICKER_DEBOUNCE_MS);

    lookup("Dup");
    lookup.cancel();
    vi.advanceTimersByTime(1000);

    expect(search).not.toHaveBeenCalled();
  });

  it("searches again for a later keystroke", () => {
    const search = vi.fn();
    const lookup = debounce(search, PICKER_DEBOUNCE_MS);

    lookup("Dup");
    vi.advanceTimersByTime(PICKER_DEBOUNCE_MS);
    lookup("Dupo");
    vi.advanceTimersByTime(PICKER_DEBOUNCE_MS);

    expect(search.mock.calls).toStrictEqual([["Dup"], ["Dupo"]]);
  });
});
