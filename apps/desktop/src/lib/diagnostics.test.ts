import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { createTranslator, lookupKey, type Catalogue } from "../i18n/translate";
import { aggregateRows, answerDiagnosticRows, codeLabel } from "./diagnostics";
import type { KnowledgeAggregates, RetrievalDiagnostics } from "./ipc";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function measured(overrides: Partial<RetrievalDiagnostics> = {}): RetrievalDiagnostics {
  return {
    path: "documents",
    plan: "whole_folder",
    selectedDocuments: 30,
    selectedWorkbooks: 0,
    chunksConsidered: 120,
    chunksSelected: 6,
    estimatedInputChars: 5400,
    inventoryMs: 12,
    scopeMs: 1,
    routingMs: 3,
    embeddingMs: 210,
    searchMs: 9,
    workbookLoadMs: 0,
    workbookCacheHit: false,
    generationFirstTokenMs: 4100,
    generationTotalMs: 23000,
    totalMs: 23500,
    targeting: null,
    ...overrides,
  };
}

function totals(overrides: Partial<KnowledgeAggregates> = {}): KnowledgeAggregates {
  return {
    questions: 12,
    withEntities: 9,
    withEntitiesShare: 0.75,
    wouldApply: 6,
    wouldApplyRate: 0.5,
    applied: 4,
    widened: 1,
    medianReductionPercent: 86,
    recallOverall: 1,
    recallWhenWouldApply: 0.875,
    missNoMention: 1,
    missCapped: 0,
    missEntityNotResolved: 0,
    skippedByReason: { small_scope: 2, ambiguous: 1 },
    ...overrides,
  };
}

function valueOf(rows: { label: string; value: string }[], label: string): string | undefined {
  return rows.find((row) => row.label === label)?.value;
}

describe("the developer panel's rows for one answer", () => {
  it("lists the path, the counts and where the time went", () => {
    const rows = answerDiagnosticRows(english, measured());

    expect(valueOf(rows, "Path")).toBe("documents");
    expect(valueOf(rows, "Plan")).toBe("whole selection");
    expect(valueOf(rows, "Selection")).toBe("30 documents, 0 workbooks");
    expect(valueOf(rows, "Passages kept / considered")).toBe("6 / 120");
    expect(valueOf(rows, "Search")).toBe("9 ms");
    expect(valueOf(rows, "First word")).toBe("4100 ms");
    expect(valueOf(rows, "Total")).toBe("23500 ms");
  });

  it("shows a dash for a time that was never reached and for a question with no plan", () => {
    const rows = answerDiagnosticRows(
      english,
      measured({ plan: null, generationFirstTokenMs: null }),
    );

    expect(valueOf(rows, "Plan")).toBe("-");
    expect(valueOf(rows, "First word")).toBe("-");
  });

  it("adds the knowledge base's part when it took part, with the reason in words", () => {
    const rows = answerDiagnosticRows(
      english,
      measured({
        targeting: {
          mode: "suggest",
          knowledgeLookupMs: 4,
          strategy: "entity_files",
          skippedReason: null,
          outcome: "shadow",
          queryEntities: [{ entityId: 7, typeId: "person", confidence: 0.9, method: "alias" }],
          candidateDocuments: 3,
          candidateWorkbooks: 0,
          reductionPercent: 90,
          widened: false,
          shadow: {
            wouldApply: true,
            cited: 2,
            citedInCandidates: 2,
            missNoMention: 0,
            missCapped: 0,
            missEntityNotResolved: 0,
          },
        },
      }),
    );

    expect(valueOf(rows, "Mode")).toBe("suggestions");
    expect(valueOf(rows, "Outcome")).toBe("measured only");
    expect(valueOf(rows, "Names recognised")).toBe("1");
    expect(valueOf(rows, "Reduction")).toBe("90 %");
    expect(valueOf(rows, "Widened")).toBe("no");
    expect(valueOf(rows, "Cited files found by the targeting")).toBe("2 of 2");
  });

  it("explains a skipped question with the reason", () => {
    const rows = answerDiagnosticRows(
      french,
      measured({
        targeting: {
          mode: "auto",
          knowledgeLookupMs: 1,
          strategy: "none",
          skippedReason: "small_scope",
          outcome: "skipped",
          queryEntities: [],
          candidateDocuments: 0,
          candidateWorkbooks: 0,
          reductionPercent: 0,
          widened: false,
          shadow: {
            wouldApply: false,
            cited: 0,
            citedInCandidates: 0,
            missNoMention: 0,
            missCapped: 0,
            missEntityNotResolved: 0,
          },
        },
      }),
    );

    expect(valueOf(rows, "Raison du non-ciblage")).toBe("sélection trop petite");
  });

  it("holds no question, no name and no file name: only labels, numbers and codes", () => {
    const rows = answerDiagnosticRows(english, measured());

    // Every value is a number with its unit, a count pair, a dash or a catalogue word.
    for (const row of rows) {
      expect(row.value, row.label).toMatch(/^(-|\d+( ms| %)?|\d+ \/ \d+|\d+ documents, \d+ workbooks|[a-z ]+)$/);
    }
  });
});

describe("the developer panel's recorded questions", () => {
  it("adds them up", () => {
    const rows = aggregateRows(english, totals());

    expect(valueOf(rows, "Questions")).toBe("12");
    expect(valueOf(rows, "With at least one name")).toBe("9 (75 %)");
    expect(valueOf(rows, "Targeting possible")).toBe("6 (50 %)");
    expect(valueOf(rows, "Median reduction")).toBe("86 %");
    expect(valueOf(rows, "Cited files found (targeting possible)")).toBe("88 %");
    expect(valueOf(rows, "Not targeted: ambiguous name")).toBe("1");
    expect(valueOf(rows, "Not targeted: selection too small")).toBe("2");
  });

  it("shows dashes where there is nothing to divide", () => {
    const rows = aggregateRows(
      english,
      totals({
        questions: 0,
        withEntitiesShare: null,
        wouldApplyRate: null,
        medianReductionPercent: null,
        recallOverall: null,
        recallWhenWouldApply: null,
        skippedByReason: {},
      }),
    );

    expect(valueOf(rows, "With at least one name")).toBe("9 (-)");
    expect(valueOf(rows, "Median reduction")).toBe("-");
    expect(valueOf(rows, "Cited files found (all questions)")).toBe("-");
  });
});

describe("a machine code as a label", () => {
  it("falls back to the code itself when the catalogue has no word for it yet", () => {
    expect(codeLabel(english, "diagnostics.reasonName", "a_reason_from_a_newer_build")).toBe(
      "a_reason_from_a_newer_build",
    );
    expect(codeLabel(english, "diagnostics.reasonName", "ambiguous")).toBe("ambiguous name");
  });

  it.each(SUPPORTED_LOCALES)(
    "%s has a word for every code Rust can send for a skipped narrowing, a path and a plan",
    (locale) => {
      const catalogue = CATALOGUES[locale] as Catalogue;
      const codes: Record<string, string[]> = {
        // `SkipReason::as_code` in `knowledge/reduce.rs`.
        "diagnostics.reasonName": [
          "off",
          "named_file",
          "small_scope",
          "no_entity",
          "no_discriminating_entity",
          "ambiguous",
          "no_candidates",
          "insignificant",
          "lookup_error",
          "user_widened",
        ],
        "diagnostics.pathName": [
          "documents",
          "tabular",
          "mixed",
          "folder_answer",
          "without_documents",
        ],
        "diagnostics.planName": ["one_file", "every_document", "whole_folder"],
        "diagnostics.modeName": ["off", "suggest", "auto"],
        "diagnostics.outcomeName": ["applied", "shadow", "skipped"],
        "diagnostics.strategyName": ["none", "entity_files", "chip_files"],
        // The type ids of the neutral base, as the name picker labels them.
        "chat.picker.type": [
          "person",
          "organization",
          "location",
          "identifier",
          "item",
          "term",
        ],
      };
      for (const [prefix, list] of Object.entries(codes)) {
        for (const code of list) {
          expect(lookupKey(catalogue, `${prefix}.${code}`), `${prefix}.${code}`).toBeDefined();
        }
      }
    },
  );
});
