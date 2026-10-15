import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { coverageLine } from "./coverage";
import type { EvidenceCoverage } from "./ipc";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function coverage(parts: Partial<EvidenceCoverage>): EvidenceCoverage {
  return {
    filesCovered: 1,
    indexedFiles: 1,
    unreadableFiles: 0,
    unvectorisedChunks: 0,
    mismatchedChunks: 0,
    partial: false,
    ...parts,
  };
}

describe("the coverage line", () => {
  it("says how much of the folder the answer rests on, in either language", () => {
    const line = coverage({ filesCovered: 10, indexedFiles: 10, unreadableFiles: 5 });

    expect(coverageLine(english, line)).toBe(
      "Based on 10 of 10 analysed documents. 5 files could not be read.",
    );
    expect(coverageLine(french, line)).toBe(
      "Réponse fondée sur 10 des 10 documents analysés. 5 fichiers n'ont pas pu être lus.",
    );
  });

  it("makes a partial answer say so rather than read as complete", () => {
    // The case reported from the workstation: six of ten, presented as "each document".
    const text = coverageLine(
      english,
      coverage({ filesCovered: 6, indexedFiles: 10, unreadableFiles: 5, partial: true }),
    );

    expect(text).toContain("6 of 10");
  });

  it("states the count even when every document was used, and never calls it complete", () => {
    const line = coverage({ filesCovered: 4, indexedFiles: 4 });

    expect(coverageLine(english, line)).toBe("Based on 4 of 4 analysed documents.");
    expect(coverageLine(french, line)).toBe("Réponse fondée sur 4 des 4 documents analysés.");
    for (const text of [coverageLine(english, line), coverageLine(french, line)]) {
      expect(text?.toLowerCase()).not.toMatch(/complete|complet/);
    }
  });

  it("says what share of a big library an ordinary answer rests on", () => {
    expect(coverageLine(english, coverage({ filesCovered: 3, indexedFiles: 885 }))).toBe(
      "Based on 3 of 885 analysed documents.",
    );
    expect(coverageLine(french, coverage({ filesCovered: 3, indexedFiles: 885 }))).toBe(
      "Réponse fondée sur 3 des 885 documents analysés.",
    );
  });

  it("says nothing for a library of one document with nothing to disclose", () => {
    expect(coverageLine(english, coverage({ filesCovered: 1, indexedFiles: 1 }))).toBeNull();
  });

  it("leaves out the unreadable sentence when a partial answer has none to disclose", () => {
    const text = coverageLine(
      english,
      coverage({ filesCovered: 3, indexedFiles: 4, partial: true }),
    );

    expect(text).toBe("Based on 3 of 4 analysed documents.");
  });

  it("uses the singular for a single document", () => {
    expect(
      coverageLine(english, coverage({ filesCovered: 1, indexedFiles: 1, unreadableFiles: 1 })),
    ).toBe("Based on 1 of 1 analysed document. 1 file could not be read.");
  });

  it("says that the search by meaning is incomplete, with the number of passages it did not see", () => {
    const line = coverage({
      filesCovered: 3,
      indexedFiles: 30,
      unvectorisedChunks: 40,
      partial: true,
    });

    expect(coverageLine(english, line)).toBe(
      "Based on 3 of 30 analysed documents. Search by meaning is incomplete: 40 passages without a vector yet.",
    );
    expect(coverageLine(french, line)).toBe(
      "Réponse fondée sur 3 des 30 documents analysés. Recherche par sens incomplète : 40 passages sans vecteur.",
    );
  });

  it("says it even for a single document, without the count that would only repeat the choice", () => {
    const text = coverageLine(
      english,
      coverage({ unvectorisedChunks: 1, partial: true }),
    );

    expect(text).toBe("Search by meaning is incomplete: 1 passage without a vector yet.");
  });

  it("says that the vectors come from another model and what to do", () => {
    const line = coverage({
      filesCovered: 2,
      indexedFiles: 30,
      mismatchedChunks: 5,
      partial: true,
    });

    expect(coverageLine(english, line)).toBe(
      "Based on 2 of 30 analysed documents. Search by meaning skipped for 5 passages: vectors from another model, press Analyse again.",
    );
    expect(coverageLine(french, line)).toBe(
      "Réponse fondée sur 2 des 30 documents analysés. Recherche par sens ignorée pour 5 passages : vecteurs d'un autre modèle, appuyez de nouveau sur Analyser.",
    );
  });
});
