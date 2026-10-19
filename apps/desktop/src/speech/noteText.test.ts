import { describe, expect, it } from "vitest";
import { appendToNote } from "./noteText";

describe("appending a passage to a note", () => {
  it("starts an empty note with the words, whatever blank comes first", () => {
    expect(appendToNote("", "Bonjour")).toStrictEqual({ text: "Bonjour", start: 0, words: "Bonjour" });
    expect(appendToNote("", "\n\n# Titre\n").text).toBe("# Titre\n");
    expect(appendToNote("  \n", "Suite").text).toBe("Suite");
  });

  it("joins two passages with one blank", () => {
    const result = appendToNote("Bonjour.", "Merci.");

    expect(result.text).toBe("Bonjour. Merci.");
    expect(result.start).toBe("Bonjour. ".length);
    expect(result.words).toBe("Merci.");
  });

  it("adds no blank after a line break or an opening mark", () => {
    expect(appendToNote("Ligne\n", "Suite").text).toBe("Ligne\nSuite");
    expect(appendToNote("Voir (", "note").text).toBe("Voir (note");
    expect(appendToNote("Il dit \"", "oui").text).toBe("Il dit \"oui");
  });

  it("glues a mark to the word before it and replaces the engine's own", () => {
    expect(appendToNote("Bonjour", ", merci").text).toBe("Bonjour, merci");
    expect(appendToNote("Bonjour.", ", merci").text).toBe("Bonjour, merci");
    expect(appendToNote("Fin", ").").text).toBe("Fin).");
  });

  it("gives a title exactly one blank line above it", () => {
    expect(appendToNote("Bonjour.", "\n\n# Titre\n").text).toBe("Bonjour.\n\n# Titre\n");
    expect(appendToNote("Bonjour.\n", "\n\n# Titre").text).toBe("Bonjour.\n\n# Titre");
    expect(appendToNote("Bonjour.\n\n", "\n\n# Titre").text).toBe("Bonjour.\n\n# Titre");
  });

  it("keeps the break asked for by a passage that is only a break", () => {
    expect(appendToNote("Bonjour", "\n").text).toBe("Bonjour  \n");
    expect(appendToNote("Bonjour\n", "\n").text).toBe("Bonjour\n");
    expect(appendToNote("Bonjour", "\n\n").text).toBe("Bonjour\n\n");
  });

  it("starts the next passage below a closed title, without a blank", () => {
    const afterTitle = appendToNote("Bonjour.", "\n\n# Visite\n").text;

    expect(appendToNote(afterTitle, "Le patient va bien.").text).toBe(
      "Bonjour.\n\n# Visite\nLe patient va bien.",
    );
  });

  it("trims blanks at the end of a line before the break", () => {
    expect(appendToNote("Bonjour  ", "\nSuite").text).toBe("Bonjour  \nSuite");
  });

  it("changes nothing for an empty passage", () => {
    expect(appendToNote("Bonjour", "")).toStrictEqual({ text: "Bonjour", start: 7, words: "" });
    expect(appendToNote("Bonjour", "  ").text).toBe("Bonjour");
  });

  it("puts the words where the text says they are", () => {
    for (const [before, piece] of [
      ["", "Un."],
      ["Un.", "Deux."],
      ["Un.", "\n\n## Sous-titre\n"],
      ["Un\n", "Deux"],
    ] as const) {
      const result = appendToNote(before, piece);

      expect(result.text.slice(result.start, result.start + result.words.length)).toBe(result.words);
      expect(result.text.startsWith(result.text.slice(0, result.start))).toBe(true);
    }
  });
});
