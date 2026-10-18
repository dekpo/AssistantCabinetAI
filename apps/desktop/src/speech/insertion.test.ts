import { describe, expect, it } from "vitest";
import { canUndo, countNumbers, insertTranscript, rememberSelection } from "./insertion";

const at = (draft: string, start: number, end = start) => rememberSelection(draft, start, end);

describe("putting the words of a dictation in the box", () => {
  it("fills an empty box with the words", () => {
    const result = insertTranscript("", "Bonjour docteur", at("", 0));

    expect(result?.text).toBe("Bonjour docteur");
    expect(result?.caret).toBe(15);
  });

  it("says where the words begin and what they are, for the name suggestions", () => {
    const draft = "Bonjour,docteur";
    const result = insertTranscript(draft, "  Dupond  ", at(draft, 8));

    expect(result?.words).toBe("Dupond");
    expect(result?.start).toBe("Bonjour, ".length);
    expect(result?.text.slice(result.start, result.start + result.words.length)).toBe("Dupond");
  });

  it("replaces the selection when the box is the one the recording started from", () => {
    const draft = "Le rendez-vous de XXX est confirmé";
    const result = insertTranscript(draft, "Marie Martin", at(draft, 18, 21));

    expect(result?.text).toBe("Le rendez-vous de Marie Martin est confirmé");
    expect(result?.caret).toBe(18 + "Marie Martin".length);
  });

  it("puts the words at the caret when nothing is selected, with a space on each side that needs one", () => {
    const draft = "Bonjourdocteur";
    const result = insertTranscript(draft, "mon", at(draft, 7));

    expect(result?.text).toBe("Bonjour mon docteur");
    expect(result?.caret).toBe("Bonjour mon ".length);
  });

  it("adds no space after a space, after an opening mark or before a closing mark", () => {
    expect(insertTranscript("Voir ", "le dossier", at("Voir ", 5))?.text).toBe("Voir le dossier");
    expect(insertTranscript("(", "urgent", at("(", 1))?.text).toBe("(urgent");
    expect(insertTranscript("Voir.", "le dossier", at("Voir.", 4))?.text).toBe("Voir le dossier.");
    expect(insertTranscript("Voir", "le dossier", at("Voir", 4))?.text).toBe("Voir le dossier");
  });

  it("appends at the end when the draft changed while she was speaking", () => {
    const remembered = at("Bonjour", 0, 7);
    const result = insertTranscript("Bonjour a tous", "merci", remembered);

    expect(result?.text).toBe("Bonjour a tous merci");
    expect(result?.caret).toBe("Bonjour a tous merci".length);
  });

  it("appends to a draft with no remembered place, adding one space only when needed", () => {
    expect(insertTranscript("Bonjour", "merci", null)?.text).toBe("Bonjour merci");
    expect(insertTranscript("Bonjour ", "merci", null)?.text).toBe("Bonjour merci");
    expect(insertTranscript("Bonjour\n", "merci", null)?.text).toBe("Bonjour\nmerci");
  });

  it("trims the transcript and changes nothing for an empty one", () => {
    expect(insertTranscript("", "  merci \n", null)?.text).toBe("merci");
    expect(insertTranscript("Bonjour", "   ", null)).toBeNull();
    expect(insertTranscript("Bonjour", "", null)).toBeNull();
  });

  it("clamps a selection that is stale or reversed", () => {
    expect(rememberSelection("abc", 9, 12)).toEqual({ draft: "abc", start: 3, end: 3 });
    expect(rememberSelection("abc", 2, 1)).toEqual({ draft: "abc", start: 1, end: 2 });
    expect(rememberSelection("abc", -4, 1)).toEqual({ draft: "abc", start: 0, end: 1 });
  });

  it("remembers the draft and caret as they were, for the one-step Undo", () => {
    const draft = "Voir XXX demain";
    const result = insertTranscript(draft, "Marie", at(draft, 5, 8));

    expect(result?.before).toEqual({ text: draft, caret: 5 });
  });
});

describe("the one-step Undo", () => {
  it("is offered only while the box holds exactly what the insertion made", () => {
    const result = insertTranscript("Bonjour", "merci", null);
    if (result === null) {
      throw new Error("an insertion was expected");
    }

    expect(canUndo(result.text, result)).toBe(true);
    expect(canUndo(`${result.text}!`, result)).toBe(false);
    expect(canUndo("", result)).toBe(false);
  });
});

describe("the numbers a dictation added", () => {
  it("counts runs of digits with their separators, once each", () => {
    expect(countNumbers("")).toBe(0);
    expect(countNumbers("Madame Martin vient mardi")).toBe(0);
    expect(countNumbers("Le 12 mars, 3 fois")).toBe(2);
    expect(countNumbers("Total 1 250,50 euros")).toBe(2);
    expect(countNumbers("Total 1 250,50 euros")).toBe(1);
    expect(countNumbers("Tel 0612345678")).toBe(1);
  });
});
