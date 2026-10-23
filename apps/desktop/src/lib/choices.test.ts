import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { choicesFromTabular, rewriteQuestion } from "./choices";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

describe("rewriteQuestion", () => {
  it("replaces the ambiguous name she wrote by the path she chose", () => {
    expect(
      rewriteQuestion(french, "Que dit neurologie.pdf ?", { path: "2026/mars/neurologie.pdf", query: "neurologie.pdf" }),
    ).toBe("Que dit 2026/mars/neurologie.pdf ?");
    expect(
      rewriteQuestion(english, "What does NEUROLOGIE.PDF say?", { path: "2026/mars/neurologie.pdf", query: "neurologie.pdf" }),
    ).toBe("What does 2026/mars/neurologie.pdf say?");
  });

  it("appends the path in her language when the name is not in her words", () => {
    expect(rewriteQuestion(french, "Que dit ce courrier ?", { path: "a/b.pdf", query: "lettre" })).toBe(
      "Que dit ce courrier ? (fichier : a/b.pdf)",
    );
    expect(rewriteQuestion(english, "What does it say?", { path: "a/b.pdf", query: "letter" })).toBe(
      "What does it say? (file: a/b.pdf)",
    );
  });

  it("names the workbook when none was named", () => {
    expect(rewriteQuestion(french, "Combien de lignes ?", { path: "factures.xlsx", query: null })).toBe(
      "Combien de lignes ? dans factures.xlsx",
    );
    expect(rewriteQuestion(english, "How many rows?", { path: "factures.xlsx", query: null })).toBe(
      "How many rows? in factures.xlsx",
    );
  });
});

describe("rewriteQuestion with the stem of a typed name", () => {
  it("does not repeat the extension she wrote after the name", () => {
    expect(
      rewriteQuestion(english, "Write a letter for each invoice of factures-2026.xlsx", {
        path: "Test/factures-2026.xlsx",
        query: "factures-2026",
      }),
    ).toBe("Write a letter for each invoice of Test/factures-2026.xlsx");
    // A name that already carries its extension is replaced as before.
    expect(
      rewriteQuestion(english, "What does neurologie.pdf say?", {
        path: "Cours/neurologie.pdf",
        query: "neurologie.pdf",
      }),
    ).toBe("What does Cours/neurologie.pdf say?");
  });
});

describe("choicesFromTabular", () => {
  it("offers every candidate of a which-workbook answer and nothing for any other answer", () => {
    expect(choicesFromTabular({ kind: "which_workbook", candidates: ["a.csv", "b.csv"] })).toEqual([
      { path: "a.csv", query: null },
      { path: "b.csv", query: null },
    ]);
    expect(
      choicesFromTabular({ kind: "fill_several_sources", candidates: ["a.csv", "b.csv"], query: "factures" }),
    ).toEqual([
      { path: "a.csv", query: "factures" },
      { path: "b.csv", query: "factures" },
    ]);
    expect(
      choicesFromTabular({ kind: "fill_several_sources", candidates: ["a.csv", "b.csv"], query: null }),
    ).toEqual([]);
    expect(choicesFromTabular({ kind: "fill_needs_template", file: "a.csv" })).toEqual([]);
    expect(choicesFromTabular({ kind: "no_usable_table" })).toEqual([]);
  });
});
