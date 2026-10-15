import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";
import { annotationLines, answerForCopy, computedBlock, sourceLines } from "./annotations";
import type { TabularAnswer, TargetingReport } from "./ipc";

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

describe("copying the sources of an answer", () => {
  const evidence = (relativePath: string, pageNumber: number, origin = "textLayer") => ({
    chunkId: `${relativePath}-${pageNumber}`,
    relativePath,
    pageNumber,
    ordinal: 1,
    section: null,
    text: "...",
    score: 1,
    origin,
    confidence: null,
  });
  const withSources = entry({
    content: "Le devis est de 1 200 euros.",
    sources: [
      evidence("devis.pdf", 1),
      evidence("devis.pdf", 3),
      evidence("scan.pdf", 2, "ocr"),
    ] as ChatEntry["sources"],
    tabularSource: { file: "factures.xlsx", sheet: "Factures", column: "montant" },
  });

  it("adds them under the answer, grouped by file, when the setting is on", () => {
    const copied = answerForCopy(french, "fr-FR", withSources, true);

    expect(copied.startsWith("Le devis est de 1 200 euros.")).toBe(true);
    expect(copied).toContain("Sources :\n- devis.pdf, pages 1, 3\n- scan.pdf, page 2");
    expect(copied).toContain("reconnaissance de texte");
    expect(copied).toContain("- factures.xlsx, feuille Factures, colonne montant");
  });

  it("copies the sheet of a table answer and of a mixed answer", () => {
    const lines = sourceLines(
      english,
      entry({ mixedTableSource: { file: "a.xlsx", sheet: "S1", column: null } }),
    );
    expect(lines[0]).toBe("Sources:");
    expect(lines).toHaveLength(2);
    expect(lines[1]).toContain("a.xlsx");
  });

  it("leaves them out when the setting is off, which is the default", () => {
    expect(answerForCopy(french, "fr-FR", withSources)).toBe("Le devis est de 1 200 euros.");
    expect(answerForCopy(french, "fr-FR", withSources, false)).not.toContain("devis.pdf");
  });

  it("adds nothing, not even the heading, to an answer that has no source", () => {
    expect(sourceLines(french, entry({}))).toEqual([]);
    expect(answerForCopy(french, "fr-FR", entry({}), true)).toBe("1 450,00 euros.");
  });
});

describe("Copy the conversation", () => {
  it("copies the text as shown, the engine's figure and the notes, and no list of sources by default", () => {
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
            ordinal: 1,
            section: null,
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

describe("Copy the conversation, when the search was narrowed", () => {
  const narrowed: TargetingReport = {
    strategy: "entity_files",
    reason: null,
    requestedBy: "auto",
    widened: "none",
    selectedDocuments: 40,
    candidateDocuments: 7,
    selectedWorkbooks: 0,
    candidateWorkbooks: 0,
    reductionPercent: 82,
    entities: [
      { entityId: 7, display: "Jean Dupont", typeId: "person", confidence: 0.9, method: "alias" },
    ],
    candidatePaths: ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt", "f.txt", "g.txt"],
    workbookPaths: [],
    ambiguities: [],
    possibles: [],
    chosenWorkbook: null,
  };

  it("copies the line that says so, because it is under the answer on screen", () => {
    const copied = answerForCopy(french, "fr-FR", entry({ targeting: narrowed }));

    expect(copied).toContain("Recherche ciblée : 7 documents sur 40 — Jean Dupont");
    // The files are a list of references, left out of a copy as the sources are.
    expect(copied).not.toContain("a.txt");
  });

  it("copies nothing more for an answer that was not narrowed", () => {
    const copied = answerForCopy(french, "fr-FR", entry({ targeting: { ...narrowed, candidatePaths: [] } }));

    expect(copied).toBe("1 450,00 euros.");
  });

  it("keeps the line out of the notes shown as lines, which the interface draws with its buttons", () => {
    expect(annotationLines(french, "fr-FR", entry({ targeting: narrowed }))).toEqual([]);
  });
});
