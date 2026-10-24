import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";
import { annotationLines, cautionLines } from "./annotations";
import { followUpOf, storedCalculationOf } from "./calculationFollowUp";
import { carriedOverLine, carryOverOf } from "./carryOver";
import type { CarriedParts, TableCarryOver, TabularAnswer } from "./ipc";
import { truncateForRegenerate } from "./turns";
import { workbookUseLines } from "./workbookUse";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function state(extra: Partial<TableCarryOver>): TableCarryOver {
  return {
    workbook: "factures.csv",
    sheet: "factures",
    column: "montant",
    operation: "sum",
    filters: [],
    groupBy: null,
    topN: null,
    descending: false,
    ...extra,
  };
}

const question = (id: string, content = "?"): ChatEntry => ({
  id,
  role: "user",
  content,
  createdAt: 0,
});
const answer = (id: string, extra: Partial<ChatEntry> = {}): ChatEntry => ({
  id,
  role: "assistant",
  content: "ok",
  createdAt: 0,
  ...extra,
});

const first = state({});
const second = state({ filters: [{ kind: "month", column: "date", month: 3 }] });
const third = state({
  filters: [
    { kind: "month", column: "date", month: 3 },
    { kind: "equals", column: "fournisseur", value: "Beta" },
  ],
});

const conversation: ChatEntry[] = [
  question("q1"),
  answer("a1", { tableCarryOver: first }),
  question("q2"),
  answer("a2", { tableCarryOver: second }),
  question("q3"),
  answer("a3", { tableCarryOver: third }),
];

describe("the state a table answer leaves for a follow-up", () => {
  it("is kept on each entry and the next question is asked with the last one", () => {
    expect(carryOverOf(conversation)).toBe(third);
    expect(carryOverOf(conversation.slice(0, 4))).toBe(second);
    expect(carryOverOf(conversation.slice(0, 2))).toBe(first);
  });

  it("is cleared with the conversation: an empty conversation leaves nothing to continue", () => {
    expect(carryOverOf([])).toBeUndefined();
    expect(carryOverOf([question("q1")])).toBeUndefined();
  });

  it("is sent only after a table answer: a document answer or a nudge in between leaves nothing", () => {
    const afterADocumentAnswer = [...conversation, question("q4"), answer("a4")];
    expect(carryOverOf(afterADocumentAnswer)).toBeUndefined();
  });

  it("is read as it was at that point when an earlier answer is regenerated or its question edited", () => {
    // Regenerating the second answer drops it and the question before it: the state is the first answer's.
    const truncated = truncateForRegenerate(conversation, "a2");
    expect(truncated).not.toBeNull();
    expect(carryOverOf(truncated?.base ?? [])).toBe(first);
    // Not the third answer's, which carries Beta.
    expect(carryOverOf(truncated?.base ?? [])).not.toBe(third);
  });
});

describe("explaining or analysing a follow-up answer", () => {
  const figure: TabularAnswer = {
    kind: "value",
    file: "factures.csv",
    value: { kind: "sum", value: { value: 33502.5, unit: "EUR", unparsed: 0 } },
    locator: {
      sheet: "factures",
      headerRow: 1,
      column: "montant",
      rowRange: [55, 59],
      filters: [{ kind: "month", column: "date", month: 3 }],
    },
    derivation: { kind: "computed", operation: "sum", row_count: 5 },
  };

  it("sends back the state the question was asked with, because the short question gives no figure alone", () => {
    const stored = storedCalculationOf(figure, undefined, false, first);
    expect(stored?.tableCarryOver).toBe(first);
    expect(followUpOf("explain", stored!).tableCarryOver).toBe(first);
    expect(followUpOf("analyse", stored!).tableCarryOver).toBe(first);
  });

  it("sends none for a question that stood alone", () => {
    const stored = storedCalculationOf(figure, undefined);
    expect(stored?.tableCarryOver).toBeUndefined();
    expect("tableCarryOver" in followUpOf("explain", stored!)).toBe(false);
  });
});

describe("the line that marks what a follow-up inherited", () => {
  const inherited: CarriedParts = {
    workbook: true,
    sheet: true,
    column: "montant",
    operation: "sum",
    groupBy: null,
    filters: [{ kind: "month", column: "date", month: 3 }],
  };

  it("is written in French from the parts", () => {
    expect(carriedOverLine(french, "fr-FR", inherited)).toBe(
      "Repris de la question précédente : le fichier ; la colonne « montant » ; la somme ; date en mars.",
    );
  });

  it("is written in English from the same parts", () => {
    expect(carriedOverLine(english, "en-US", inherited)).toBe(
      'Taken from the previous question: the file; the column "montant"; the sum; date in March.',
    );
  });

  it("says only the file when nothing else was inherited", () => {
    const only: CarriedParts = { ...inherited, column: null, operation: null, filters: [] };
    expect(carriedOverLine(english, "en-US", only)).toBe("Taken from the previous question: the file.");
  });

  it("is a note under the answer and not a caution read aloud", () => {
    const entry = answer("a2", { tabular: true, carriedOver: inherited });
    expect(annotationLines(english, "en-US", entry)).toContain(
      'Taken from the previous question: the file; the column "montant"; the sum; date in March.',
    );
    expect(cautionLines(english, "en-US", entry)).toEqual([]);
  });

  it("has a sentence for every operation in both languages", () => {
    const operations: CarriedParts["operation"][] = [
      "count",
      "distinct",
      "sum",
      "min",
      "max",
      "mean",
      "median",
      "group_sum",
      "largest_group",
      "least_group",
      "top_groups",
      "count_per_group",
      "largest_row",
      "filter",
      "sort",
    ];
    for (const operation of operations) {
      for (const [t, locale] of [
        [english, "en-US"],
        [french, "fr-FR"],
      ] as const) {
        const line = carriedOverLine(t, locale, { ...inherited, operation, column: null, filters: [] });
        expect(line, `${locale} ${operation}`).not.toContain("tabularAnswer.");
      }
    }
  });
});

describe("the trace lines of a follow-up", () => {
  const use = (reason: "carried_over" | "carry_over_dropped", how: "computed" | "not_used") => ({
    file: "factures.csv",
    how,
    reason,
    rowsSent: null,
    rowsTotal: null,
    rowsTruncated: false,
    truncated: false,
    count: null,
    candidates: [],
  });

  it("marks a continued workbook as informative", () => {
    const [line] = workbookUseLines(english, [use("carried_over", "computed")]);
    expect(line?.text).toContain("continuing the previous question");
    expect(line?.caution).toBe(false);
  });

  it("says a dropped state in a caution, in both languages", () => {
    const [en] = workbookUseLines(english, [use("carry_over_dropped", "not_used")]);
    const [fr] = workbookUseLines(french, [use("carry_over_dropped", "not_used")]);
    expect(en?.caution).toBe(true);
    expect(en?.text).toContain("your question is read on its own");
    expect(fr?.caution).toBe(true);
    expect(fr?.text).toContain("votre question est lue seule");
  });
});
