import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";
import { annotationLines, answerForCopy, cautionLines, computedBlock, sourceItems, sourceLines } from "./annotations";
import type { TabularAnswer, TargetingReport, WorkbookUse } from "./ipc";

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
    expect(french("chat.deterministicData")).toContain("dossier des tableaux");
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

  it("cites a file of the other folder by its path, never by its key", () => {
    const foreign = entry({
      sources: [evidence("/data/Test/modele_lettre.docx", 2)] as ChatEntry["sources"],
      tabularSource: { file: "/documents/administratif/commandes.xlsx", sheet: "S1", column: null },
    });
    const lines = sourceLines(english, foreign).join(" ");

    expect(lines).toContain("Test/modele_lettre.docx");
    expect(lines).toContain("administratif/commandes.xlsx");
    expect(lines).not.toContain("/data/");
    expect(lines).not.toContain("/documents/");
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

  it("lists a workbook whose structure the model was given, by its plain path, beside the other sources", () => {
    const lines = sourceLines(
      english,
      entry({
        workbookCards: [
          { file: "factures.csv", rowsSent: null, rowsTotal: null },
          { file: "/documents/administratif/commandes.xlsx", rowsSent: null, rowsTotal: null },
        ],
      }),
    );
    expect(lines[0]).toBe("Sources:");
    expect(lines).toHaveLength(3);
    expect(lines[1]).toBe("- factures.csv (structure only, no data read by the AI)");
    expect(lines[2]).toContain("administratif/commandes.xlsx");
    expect(lines[2]).not.toContain("/documents/");
    expect(
      sourceLines(french, entry({ workbookCards: [{ file: "factures.csv", rowsSent: null, rowsTotal: null }] }))[1],
    ).toContain("structure seulement");
  });

  it("says how many rows of a workbook the AI was shown, and that identifiers were masked", () => {
    const shown = (rowsSent: number, rowsTotal: number) =>
      entry({ workbookCards: [{ file: "factures.csv", rowsSent, rowsTotal }] });

    expect(sourceLines(english, shown(28, 60))[1]).toBe(
      "- factures.csv (structure and 28 rows of 60, identifiers masked)",
    );
    expect(sourceLines(french, shown(28, 60))[1]).toBe(
      "- factures.csv (structure et 28 lignes sur 60, identifiants masqués)",
    );
    expect(sourceLines(english, shown(1, 1))[1]).toBe(
      "- factures.csv (structure and 1 row of 1, identifiers masked)",
    );
    // No row sent but the figures of the whole table were: said so.
    expect(sourceLines(english, shown(0, 60))[1]).toBe(
      "- factures.csv (structure and column figures, identifiers masked)",
    );
    expect(sourceLines(french, shown(0, 60))[1]).toBe(
      "- factures.csv (structure et chiffres de colonnes, identifiants masqués)",
    );
    // Nothing of the data was sent: the structure alone.
    expect(
      sourceLines(english, entry({ workbookCards: [{ file: "factures.csv", rowsSent: null, rowsTotal: null }] }))[1],
    ).toContain("structure only");
  });

  it("gives the screen and the copy one list, documents first, then the sheet, then the workbooks read as a card", () => {
    const items = sourceItems(
      english,
      entry({
        sources: [
          {
            chunkId: "c1",
            relativePath: "contract.pdf",
            pageNumber: 2,
            ordinal: 1,
            section: null,
            text: "t",
            score: 1,
            origin: "textLayer",
            confidence: null,
          },
        ],
        mixedTableSource: { file: "a.xlsx", sheet: "S1", column: "amount" },
        workbookCards: [{ file: "b.csv", rowsSent: null, rowsTotal: null }],
      }),
    );
    expect(items).toHaveLength(3);
    expect(items[0]).toContain("contract.pdf");
    expect(items[1]).toContain("a.xlsx");
    expect(items[2]).toContain("b.csv");
    expect(
      sourceLines(english, entry({ workbookCards: [{ file: "b.csv", rowsSent: null, rowsTotal: null }] })),
    ).toEqual([
      "Sources:",
      `- ${items[2]}`,
    ]);
    expect(sourceItems(english, entry({}))).toEqual([]);
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

describe("the caution lines read aloud", () => {
  const everything = entry({
    coverage: undefined,
    withoutDocuments: true,
    unanalysedFiles: 2,
    scopeOutdated: ["a.pdf"],
    interrupted: "stopped",
    durationMs: 44000,
    modelAlias: "gemma2:2b",
    deterministic: true,
    documentsNotNeeded: true,
    mixedTableUnavailable: "no_evidence",
    mixedDocumentsUnavailable: "not_asked_about",
    mixedCorrections: [{ claimed: "1 480", correct: 1450 }],
    mixedRejectedCitations: ["[3]"],
    modelAttemptAlias: "ministral",
    modelAttemptDurationMs: 2000,
    unverifiedNumbers: ["80"],
    mixedModelArithmetic: ["10 + 40 = 50"],
  });

  it("keeps the warnings, in the order annotationLines writes them, and leaves out the rest", () => {
    const cautions = cautionLines(english, "en-US", everything);

    expect(cautions).toStrictEqual([
      english("chat.withoutDocuments"),
      english("chat.unanalysed", { documents: "2 unanalysed documents" }),
      english("chat.scopeOutdated", { files: "1 file" }),
      english("chat.interrupted"),
      english("chat.mixedTableUnavailable.no_evidence"),
      english("chat.mixedCorrection", { claimed: "1 480", correct: "1,450" }),
      english("chat.mixedRejectedCitation", { citation: "[3]" }),
      english("chat.unverifiedNumber", { number: "80" }),
      english("chat.modelArithmetic", { expression: "10 + 40 = 50" }),
    ]);
    expect(cautions.at(-1)).toBe(
      "The AI did a calculation itself (“10 + 40 = 50”): only the engine is reliable. Ask for this total to get it.",
    );
    expect(cautionLines(french, "fr-FR", everything).at(-1)).toBe(
      "L'IA a effectué elle-même un calcul (« 10 + 40 = 50 ») : seul le moteur est fiable. Demandez ce total pour l'obtenir.",
    );
    // Every caution line is one of the lines shown under the answer, in the same order.
    const shown = annotationLines(english, "en-US", everything);
    expect(shown.filter((line) => cautions.includes(line))).toStrictEqual(cautions);
  });

  it("leaves out the variant that only says the question did not concern a side, and the coverage", () => {
    const quiet = entry({
      mixedTableUnavailable: "not_asked_about",
      mixedDocumentsUnavailable: "not_asked_about",
    });

    expect(cautionLines(english, "en-US", quiet)).toStrictEqual([]);
    expect(annotationLines(english, "en-US", quiet)).toHaveLength(2);
  });

  it("is empty for a plain answer", () => {
    expect(cautionLines(english, "en-US", entry({ durationMs: 1000, modelAlias: "m" }))).toStrictEqual([]);
  });
});

function use(extra: Partial<WorkbookUse>): WorkbookUse {
  return {
    file: "ventes.csv",
    how: "card_sent",
    reason: null,
    rowsSent: null,
    rowsTotal: null,
    truncated: false,
    count: null,
    candidates: [],
    ...extra,
  };
}

describe("the lines about the workbooks of an answer", () => {
  const trace: WorkbookUse[] = [
    use({ file: "ventes.csv" }),
    use({ file: "/documents/Factures/2025.xlsx", reason: "implied_by_word", truncated: true }),
    use({ file: "stock.csv", how: "not_used", reason: "named_not_selected" }),
    use({ file: "", how: "not_used", reason: "not_relevant", count: 2 }),
  ];

  it("come last among the notes, the same in the copy as on the screen", () => {
    const shown = annotationLines(
      french,
      "fr-FR",
      entry({ durationMs: 1000, modelAlias: "m", workbookUse: trace }),
    );

    expect(shown).toHaveLength(5);
    expect(shown[1]).toBe(french("workbookUse.cardSent.named", { path: "ventes.csv" }));
    expect(shown.at(-1)).toBe(french("workbookUse.notUsed.not_relevant_many", { count: "2" }));
    expect(answerForCopy(french, "fr-FR", entry({ workbookUse: trace }))).toContain(shown[2]);
  });

  it("read aloud: a refusal and a shrunk card, not the card sent, the figure or the count left out", () => {
    const cautions = cautionLines(english, "en-US", entry({ workbookUse: trace }));

    expect(cautions).toStrictEqual([
      `${english("workbookUse.cardSent.implied_by_word", { path: "Factures/2025.xlsx" })}${english("workbookUse.cardTruncated")}`,
      english("workbookUse.notUsed.named_not_selected", { path: "stock.csv" }),
    ]);
  });

  it("say no document was selected for an answer written from the workbooks alone, and warn of nothing", () => {
    const written = entry({ mixedDocumentsUnavailable: "not_selected" });

    expect(annotationLines(english, "en-US", written)).toStrictEqual([
      english("chat.mixedDocumentsUnavailable.not_selected"),
    ]);
    expect(cautionLines(english, "en-US", written)).toStrictEqual([]);
    expect(english("chat.mixedDocumentsUnavailable.not_selected")).toBe(
      "You selected no document: this answer rests on your spreadsheets only.",
    );
    expect(annotationLines(french, "fr-FR", written)).toStrictEqual([
      french("chat.mixedDocumentsUnavailable.not_selected"),
    ]);
  });

  it("replace the old sentence once they say what happened to the workbooks", () => {
    const quiet = entry({ mixedTableUnavailable: "not_asked_about" });

    expect(annotationLines(english, "en-US", quiet)).toStrictEqual([
      english("chat.mixedTableUnavailable.not_asked_about"),
    ]);
    expect(
      annotationLines(
        english,
        "en-US",
        entry({
          mixedTableUnavailable: "not_asked_about",
          workbookUse: [use({ file: "", how: "not_used", reason: "not_relevant", count: 3 })],
        }),
      ),
    ).toStrictEqual([english("workbookUse.notUsed.not_relevant_many", { count: "3" })]);
    expect(english("chat.mixedTableUnavailable.not_asked_about")).toBe(
      "No spreadsheet was needed for this question.",
    );
  });
});
