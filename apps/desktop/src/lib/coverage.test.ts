import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { coverageLine } from "./coverage";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

describe("the coverage line", () => {
  it("says how much of the folder the answer rests on, in either language", () => {
    const coverage = { filesCovered: 10, indexedFiles: 10, unreadableFiles: 5 };

    expect(coverageLine(english, coverage)).toBe(
      "Based on 10 of 10 analysed documents. 5 files could not be read.",
    );
    expect(coverageLine(french, coverage)).toBe(
      "Réponse fondée sur 10 des 10 documents analysés. 5 fichiers n'ont pas pu être lus.",
    );
  });

  it("makes a partial answer say so rather than read as complete", () => {
    // The case reported from the workstation: six of ten, presented as "each document".
    const text = coverageLine(english, {
      filesCovered: 6,
      indexedFiles: 10,
      unreadableFiles: 5,
    });

    expect(text).toContain("6 of 10");
  });

  it("says nothing when the answer is complete and nothing is unreadable", () => {
    expect(
      coverageLine(english, { filesCovered: 4, indexedFiles: 4, unreadableFiles: 0 }),
    ).toBeNull();
  });

  it("leaves out the unreadable sentence when a partial answer has none to disclose", () => {
    const text = coverageLine(english, {
      filesCovered: 3,
      indexedFiles: 4,
      unreadableFiles: 0,
    });

    expect(text).toBe("Based on 3 of 4 analysed documents.");
  });

  it("uses the singular for a single document", () => {
    expect(
      coverageLine(english, { filesCovered: 1, indexedFiles: 1, unreadableFiles: 1 }),
    ).toBe("Based on 1 of 1 analysed document. 1 file could not be read.");
  });
});
