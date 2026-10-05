import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";
import { annotationLines, answerForCopy, computedBlock } from "./annotations";
import type { TabularAnswer } from "./ipc";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

const total: TabularAnswer = {
  kind: "value",
  file: "factures.xlsx",
  value: { kind: "sum", value: { value: 1450, unit: null, unparsed: 0 } },
  locator: {
    sheet: "Factures",
    headerRow: 0,
    column: "montant",
    rowRange: [0, 7],
    filters: [{ kind: "equals", column: "fournisseur", value: "MedSupply" }],
  },
  derivation: { kind: "computed", operation: "sum", row_count: 3 },
};

function entry(extra: Partial<ChatEntry>): ChatEntry {
  return { id: "a", role: "assistant", content: "1 450,00 euros.", createdAt: 0, ...extra };
}

describe("what is written under an answer", () => {
  it("lists, in order, where it came from and what could not be established", () => {
    const lines = annotationLines(
      french,
      "fr-FR",
      entry({
        durationMs: 44000,
        modelAlias: "gemma2:2b",
        mixed: true,
        mixedTable: total,
        mixedDocumentsUnavailable: "gateway_unavailable",
        mixedCorrections: [{ claimed: "1 480", correct: 1450 }],
      }),
    );
    expect(lines[0]).toContain("gemma2:2b");
    expect(lines.some((line) => line.includes("n'a pas pu être contactée"))).toBe(true);
    expect(lines.some((line) => line.includes("1 480"))).toBe(true);
  });

  it("names the data folder for a table answer given inside a mixed question", () => {
    const lines = annotationLines(french, "fr-FR", entry({ deterministic: true, mixed: true }));
    expect(lines).toEqual([french("chat.deterministicData")]);
    expect(french("chat.deterministicData")).toContain("dossier des données");
  });

  it("says a figure the model stated is in none of the excerpts", () => {
    const lines = annotationLines(french, "fr-FR", entry({ unverifiedNumbers: ["80"] }));
    expect(lines).toHaveLength(1);
    expect(lines[0]).toContain("« 80 »");
    expect(annotationLines(english, "en-US", entry({ unverifiedNumbers: ["80"] }))[0]).toContain("“80”");
  });

  it("has nothing to say about an answer that rests on nothing special", () => {
    expect(annotationLines(english, "en-US", entry({}))).toEqual([]);
  });
});

describe("the engine's own figure beside a model's prose", () => {
  it("is a title and the sentence the engine would have given, with its filters", () => {
    const block = computedBlock(french, "fr-FR", entry({ mixed: true, mixedTable: total }));
    expect(block).toContain("par le moteur, pas par l'IA");
    expect(block).toContain("Somme de montant");
    expect(block).toContain("fournisseur = MedSupply");
  });

  it("is absent when the table's sentence already is the whole answer", () => {
    expect(computedBlock(french, "fr-FR", entry({ mixed: true, mixedTable: total, deterministic: true }))).toBeNull();
    expect(computedBlock(french, "fr-FR", entry({}))).toBeNull();
  });
});

describe("Copy the conversation", () => {
  it("copies the text as shown, the engine's figure and the notes, and no list of sources", () => {
    const copied = answerForCopy(
      french,
      "fr-FR",
      entry({
        content: "Aucun document dans le WORK_FOLDER_CONTEXT ne le dit. 1 450,00 euros.",
        mixed: true,
        mixedTable: total,
        durationMs: 34000,
        modelAlias: "gemma2:2b",
        sources: [
          {
            chunkId: "c",
            relativePath: "devis.pdf",
            pageNumber: 1,
            section: 1,
            text: "Montant total du devis : 1 200,00 euros HT.",
            score: 1,
            origin: "textLayer",
            confidence: null,
          },
        ],
      }),
    );
    expect(copied).toContain("Dossier des documents");
    expect(copied).not.toContain("WORK_FOLDER_CONTEXT");
    expect(copied).toContain("Somme de montant");
    expect(copied).toContain("gemma2:2b");
    expect(copied).not.toContain("devis.pdf");
    expect(copied).not.toContain("Montant total du devis");
  });
});
