import { describe, expect, it } from "vitest";
import type { RepairSet } from "./ipc";
import {
  acceptChip,
  chipsFromSet,
  followEdit,
  replacementFor,
  withoutChip,
  type RepairChip,
  type RepairSource,
} from "./repairChips";

const DUPONT = { entityId: 7, display: "Jean Dupont" };
const DURAND = { entityId: 8, display: "Marc Durand" };
const DURANT = { entityId: 9, display: "Eric Durant" };

function sourceOf(draft: string, words: string): RepairSource {
  return { id: {}, words, start: draft.indexOf(words), draft, caseReliable: true };
}

function setOf(words: string, heard: string, candidates = [DUPONT]): RepairSet {
  const start = words.indexOf(heard);
  return {
    proposals: [
      {
        start,
        end: start + heard.length,
        heard,
        action: candidates.length === 1 ? "suggest" : "ask",
        candidates,
      },
    ],
    hiddenCount: 0,
  };
}

function chipFor(draft: string, heard: string, candidates = [DUPONT]): RepairChip {
  const source = sourceOf(draft, draft);
  const chips = chipsFromSet(setOf(draft, heard, candidates), source, 1);
  expect(chips).toHaveLength(1);
  return chips[0] as RepairChip;
}

describe("placing the suggestions of a dictation in the draft", () => {
  it("shifts the spans of the transcript to where the words went in the draft", () => {
    const draft = "Question: J'ai vu Dupond hier";
    const words = "J'ai vu Dupond hier";

    const [chip] = chipsFromSet(setOf(words, "Dupond"), sourceOf(draft, words), 3);

    expect(chip?.id).toBe("3-0");
    expect(draft.slice(chip?.start, chip?.end)).toBe("Dupond");
  });

  it("drops a suggestion whose words are not where Rust said", () => {
    const draft = "Question: J'ai vu Dupond hier";
    const words = "J'ai vu Dupond hier";
    const wrong = setOf(words, "Dupond");
    wrong.proposals[0] = { ...wrong.proposals[0]!, start: 0, end: 6 };

    expect(chipsFromSet(wrong, sourceOf(draft, words), 1)).toEqual([]);
  });

  it("keeps the alternatives of a question in the order Rust gave them", () => {
    const draft = "Duran";
    const [chip] = chipsFromSet(setOf(draft, "Duran", [DURAND, DURANT]), sourceOf(draft, draft), 1);

    expect(chip?.action).toBe("ask");
    expect(chip?.candidates.map((candidate) => candidate.display)).toEqual([
      "Marc Durand",
      "Eric Durant",
    ]);
  });

  it("counts UTF-16 units, so an accent or an emoji before the name does not shift it", () => {
    const draft = "\u{1F600} Lèvre et Dupond";
    const words = draft;
    const start = draft.indexOf("Dupond");

    const [chip] = chipsFromSet(
      {
        proposals: [{ start, end: start + 6, heard: "Dupond", action: "suggest", candidates: [DUPONT] }],
        hiddenCount: 0,
      },
      sourceOf(draft, words),
      1,
    );

    expect(draft.slice(chip?.start, chip?.end)).toBe("Dupond");
  });
});

describe("a suggestion follows the edits of the draft", () => {
  const draft = "Voir Dupond demain";

  it("stays put when the text after it changes", () => {
    const chip = chipFor(draft, "Dupond");

    const kept = followEdit([chip], draft, "Voir Dupond demain matin");

    expect(kept).toEqual([chip]);
  });

  it("moves when text is typed before it", () => {
    const chip = chipFor(draft, "Dupond");
    const after = "Bonjour. Voir Dupond demain";

    const [moved] = followEdit([chip], draft, after);

    expect(after.slice(moved?.start, moved?.end)).toBe("Dupond");
  });

  it("moves back when text before it is deleted", () => {
    const chip = chipFor("Merci beaucoup. Voir Dupond", "Dupond");
    const after = "Voir Dupond";

    const [moved] = followEdit([chip], "Merci beaucoup. Voir Dupond", after);

    expect(after.slice(moved?.start, moved?.end)).toBe("Dupond");
  });

  it("is dropped when the words themselves are edited", () => {
    const chip = chipFor(draft, "Dupond");

    expect(followEdit([chip], draft, "Voir Dupon demain")).toEqual([]);
    expect(followEdit([chip], draft, "Voir Dupond2 demain")).toEqual([]);
  });

  it("is dropped when the draft is replaced or emptied", () => {
    const chip = chipFor(draft, "Dupond");

    expect(followEdit([chip], draft, "")).toEqual([]);
    expect(followEdit([chip], draft, "Autre chose")).toEqual([]);
  });

  it("is dropped when the same words no longer sit at its place", () => {
    const chip = chipFor("Dupond et Dupond", "Dupond");

    // The first "Dupond" is deleted: what was the second now sits at the start. The chip belonged
    // to the first, so it must not jump onto the other.
    const moved = followEdit([chip], "Dupond et Dupond", "et Dupond");

    expect(moved.every((kept) => "et Dupond".slice(kept.start, kept.end) === "Dupond")).toBe(true);
  });

  it("keeps the other suggestions when one is dropped", () => {
    const text = "Dupond et Lefevre";
    const first = chipFor(text, "Dupond");
    const second = { ...chipFor(text, "Lefevre"), id: "1-1" };

    const kept = followEdit([first, second], text, "Dupon et Lefevre");

    expect(kept.map((chip) => chip.heard)).toEqual(["Lefevre"]);
  });
});

describe("accepting a suggestion", () => {
  it("replaces only the words heard, with the name as the base writes it", () => {
    const draft = "Voir Dupond demain";
    const chip = chipFor(draft, "Dupond");

    const replacement = replacementFor(draft, chip, DUPONT);

    expect(replacement?.text).toBe("Voir Jean Dupont demain");
    expect(replacement?.caret).toBe("Voir Jean Dupont".length);
  });

  it("applies the name that was picked among several", () => {
    const draft = "Voir Duran demain";
    const chip = chipFor(draft, "Duran", [DURAND, DURANT]);

    expect(replacementFor(draft, chip, DURANT)?.text).toBe("Voir Eric Durant demain");
  });

  it("does nothing when the draft no longer holds the words at that place", () => {
    const chip = chipFor("Voir Dupond demain", "Dupond");

    expect(replacementFor("Voir Dupont demain", chip, DUPONT)).toBeNull();
    expect(replacementFor("Dupond", chip, DUPONT)).toBeNull();
    expect(replacementFor("", chip, DUPONT)).toBeNull();
  });

  it("never reads or changes anything but the text it is given", () => {
    const draft = "Voir Dupond demain";
    const chip = chipFor(draft, "Dupond");
    const frozen = Object.freeze({ ...chip, candidates: Object.freeze([DUPONT]) as never });

    expect(() => replacementFor(draft, frozen, DUPONT)).not.toThrow();
    expect(draft).toBe("Voir Dupond demain");
  });
});

describe("any kind of name", () => {
  it("is handled the same: a suggestion is an id and a display, never a type", () => {
    const organisation = { entityId: 21, display: "Mutuelle Santelis" };
    const anything = { entityId: 22, display: "Any Thing 500" };
    const draft = "Facture Santelys et Anything";

    for (const [heard, choice] of [
      ["Santelys", organisation],
      ["Anything", anything],
    ] as const) {
      const chip = chipFor(draft, heard, [choice]);
      expect(replacementFor(draft, chip, choice)?.text).toBe(draft.replace(heard, choice.display));
    }
  });
});

describe("pressing a name or Ignore", () => {
  const text = "Dupond et Lefevre";
  const first = chipFor(text, "Dupond");
  const second = { ...chipFor(text, "Lefevre", [DURAND]), id: "1-1" };

  it("replaces the words and takes only that suggestion away", () => {
    const accepted = acceptChip([first, second], text, first.id, DUPONT.entityId);

    expect(accepted.replacement?.text).toBe("Jean Dupont et Lefevre");
    expect(accepted.chips).toEqual([second]);
  });

  it("takes the suggestion away without changing the text when the words are gone", () => {
    const accepted = acceptChip([first, second], "Dupon et Lefevre", first.id, DUPONT.entityId);

    expect(accepted.replacement).toBeNull();
    expect(accepted.chips).toEqual([second]);
  });

  it("ignores a name that the suggestion never offered, and a suggestion that does not exist", () => {
    expect(acceptChip([first], text, first.id, 999)).toEqual({ chips: [first], replacement: null });
    expect(acceptChip([first], text, "nope", DUPONT.entityId)).toEqual({
      chips: [first],
      replacement: null,
    });
  });

  it("Ignore leaves the text alone and the others in place", () => {
    expect(withoutChip([first, second], first.id)).toEqual([second]);
    expect(withoutChip([first, second], "nope")).toEqual([first, second]);
  });

  it("the second suggestion still lands on its words after the first was accepted", () => {
    const accepted = acceptChip([first, second], text, first.id, DUPONT.entityId);
    const moved = followEdit(accepted.chips, text, accepted.replacement?.text ?? text);
    const again = acceptChip(moved, accepted.replacement?.text ?? text, second.id, DURAND.entityId);

    expect(again.replacement?.text).toBe("Jean Dupont et Marc Durand");
  });
});
