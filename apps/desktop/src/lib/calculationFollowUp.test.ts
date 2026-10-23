import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { readingParts } from "../speech/output/readingParts";
import type { ChatEntry } from "../state/useChat";
import { annotationLines, answerForCopy, cautionLines } from "./annotations";
import {
  appendExplanation,
  beginExplanation,
  buttonsFor,
  explanationBlock,
  followUpOf,
  settleExplanation,
  storedCalculationOf,
  stoppedExplanation,
  withExplanation,
  type StoredCalculation,
} from "./calculationFollowUp";
import type { TabularAnswer } from "./ipc";
import { workbookUseLines } from "./workbookUse";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

const total: TabularAnswer = {
  kind: "value",
  file: "commandes.xlsx",
  value: { kind: "sum", value: { value: 8412.5, unit: "EUR", unparsed: 0 } },
  locator: { sheet: "Commandes", headerRow: 0, column: "Montant", rowRange: [0, 40], filters: [] },
  derivation: { kind: "computed", operation: "sum", row_count: 41 },
};

const interpreted: TabularAnswer = {
  ...total,
  derivation: {
    kind: "interpreted_by_model",
    operation: "sum",
    row_count: 41,
    model_alias: "cabinet-chat",
    plan: { aggregate: { op: "sum", column: "Montant" } },
  },
} as TabularAnswer;

const scope = { mode: { kind: "whole_folder" } } as never;

const calculation = storedCalculationOf(total, scope) as StoredCalculation;

function entry(extra: Partial<ChatEntry>): ChatEntry {
  return { id: "a", role: "assistant", content: "8 412,50 EUR", createdAt: 0, ...extra };
}

describe("the buttons under an answer", () => {
  it("gives a computed value in tables only two buttons: explain the calculation, ask the AI (the analysis)", () => {
    expect(
      buttonsFor(entry({ tabular: true, deterministic: true, tabularRetryable: true, calculation })),
    ).toEqual({ explain: true, analyse: true, regenerate: null });
  });

  it("gives the same two buttons to a figure the hidden interpreter read (no `deterministic` flag)", () => {
    const stored = storedCalculationOf(interpreted, scope) as StoredCalculation;
    expect(buttonsFor(entry({ tabular: true, tabularRetryable: true, calculation: stored }))).toEqual({
      explain: true,
      analyse: true,
      regenerate: null,
    });
  });

  it("gives the data-only answer of a mixed selection the same two buttons", () => {
    expect(
      buttonsFor(
        entry({
          tabular: true,
          deterministic: true,
          tabularRetryable: true,
          documentsNotNeeded: true,
          calculation,
        }),
      ),
    ).toEqual({ explain: true, analyse: true, regenerate: null });
  });

  it("keeps the old meaning of the one button on a nudge: the AI tries to understand the question", () => {
    expect(
      buttonsFor(entry({ tabular: true, deterministic: true, tabularRetryable: true, tabularNudge: true })),
    ).toEqual({ explain: false, analyse: false, regenerate: "actions.regenerateWithModel" });
  });

  it("gives a structural fact and a human choice no button, as before", () => {
    expect(buttonsFor(entry({ tabular: true, deterministic: true }))).toEqual({
      explain: false,
      analyse: false,
      regenerate: null,
    });
  });

  it("gives a mixed answer that carries a computed value `Regenerer` and `Expliquer`, not a second `Regenerer`", () => {
    expect(
      buttonsFor(entry({ mixed: true, mixedTable: total, calculation, mixedDocumentsUnavailable: undefined })),
    ).toEqual({ explain: true, analyse: false, regenerate: "actions.regenerate" });
  });

  it("gives the figure a mixed answer shows alone (the model wrote nothing) the two buttons", () => {
    expect(
      buttonsFor(entry({ mixed: true, deterministic: true, mixedTable: total, calculation })),
    ).toEqual({ explain: true, analyse: true, regenerate: null });
  });

  it("gives a tables-only advisory answer the one button `Regenerer`, and neither new action", () => {
    expect(
      buttonsFor(entry({ mixed: true, mixedDocumentsUnavailable: "not_selected" })),
    ).toEqual({ explain: false, analyse: false, regenerate: "actions.regenerate" });
  });

  it("leaves a document answer and a folder answer as they were", () => {
    expect(buttonsFor(entry({}))).toEqual({
      explain: false,
      analyse: false,
      regenerate: "actions.regenerate",
    });
    expect(buttonsFor(entry({ deterministic: true }))).toEqual({
      explain: false,
      analyse: false,
      regenerate: "actions.regenerateWithModel",
    });
  });

  it("offers nothing to explain when the answer kept no figure", () => {
    expect(buttonsFor(entry({ tabular: true, tabularRetryable: true, deterministic: true }))).toEqual({
      explain: false,
      analyse: false,
      regenerate: "actions.regenerateWithModel",
    });
  });
});

describe("the figure an answer keeps", () => {
  it("stores the figure as the request wants it back, with the selection the answer was given under", () => {
    expect(calculation.shown).toEqual({
      file: "commandes.xlsx",
      value: total.kind === "value" ? total.value : null,
      derivation: { kind: "computed", operation: "sum", row_count: 41 },
    });
    expect(calculation.scope).toBe(scope);
    expect(followUpOf("explain", calculation)).toEqual({ kind: "explain", shown: calculation.shown });
  });

  it("keeps the plan of a figure the model interpreted, to be replayed", () => {
    const stored = storedCalculationOf(interpreted, scope) as StoredCalculation;
    expect(stored.shown.derivation).toMatchObject({ kind: "interpreted_by_model", plan: { aggregate: {} } });
  });

  it("stores nothing for an answer that is not a computed value", () => {
    expect(storedCalculationOf({ kind: "no_usable_table" }, scope)).toBeUndefined();
    expect(storedCalculationOf(null, scope)).toBeUndefined();
  });

  it("remembers that an answer is already the analysis, so asking again asks for the analysis", () => {
    expect(storedCalculationOf(total, scope, true)?.analysed).toBe(true);
    expect(storedCalculationOf(total, scope)?.analysed).toBeUndefined();
  });
});

describe("the explanation block", () => {
  it("is added under the figure and replaced, never stacked, when the button is pressed again", () => {
    let current = entry({ calculation });
    current = withExplanation(current, beginExplanation());
    current = withExplanation(current, appendExplanation(current.explanation, " The column is "));
    current = withExplanation(current, appendExplanation(current.explanation, "Montant."));
    expect(current.explanation?.text).toBe("The column is Montant.");
    expect(current.explanation?.status).toBe("running");

    // Pressed again: a fresh block, the text of the first one is gone.
    current = withExplanation(current, beginExplanation());
    expect(current.explanation).toEqual(beginExplanation());
    expect(current.content).toBe("8 412,50 EUR");
    expect(current.calculation).toBe(calculation);
  });

  it("drops a leading space of the first piece only", () => {
    const first = appendExplanation(undefined, " Bonjour");
    expect(first.text).toBe("Bonjour");
    expect(appendExplanation(first, " le").text).toBe("Bonjour le");
  });

  it("settles on the checked text, or on `unavailable` when the AI machine could not be reached", () => {
    expect(
      settleExplanation({ text: "Done.", unavailable: false, unverifiedNumbers: ["7777"], modelArithmetic: [] }),
    ).toEqual({ text: "Done.", status: "done", unverifiedNumbers: ["7777"], modelArithmetic: [] });
    expect(
      settleExplanation({ text: "", unavailable: true, unverifiedNumbers: [], modelArithmetic: [] }).status,
    ).toBe("unavailable");
  });

  it("keeps what was written when she stops it, and drops an empty block", () => {
    const written = appendExplanation(beginExplanation(), "Half an explanation");
    expect(stoppedExplanation(written)).toEqual({ ...written, status: "done" });
    expect(stoppedExplanation(beginExplanation())).toBeUndefined();
    expect(stoppedExplanation(undefined)).toBeUndefined();
  });

  it("is copied with the answer, in her language, and read aloud after the figure", () => {
    const done = entry({
      mixed: true,
      mixedTable: total,
      calculation,
      explanation: settleExplanation({
        text: "La somme de la colonne Montant.",
        unavailable: false,
        unverifiedNumbers: [],
        modelArithmetic: [],
      }),
    });
    const copied = answerForCopy(french, "fr-FR", done);
    expect(copied).toContain(french("chat.explanationTitle"));
    expect(copied).toContain("La somme de la colonne Montant.");
    expect(copied.indexOf("Calculé dans vos données")).toBeLessThan(copied.indexOf("La somme de la colonne"));

    const parts = readingParts(french, "fr-FR", done);
    expect(parts.some((part) => part.text.includes("La somme de la colonne Montant."))).toBe(true);
  });

  it("is not copied or read while it runs", () => {
    const running = entry({ calculation, explanation: beginExplanation() });
    expect(explanationBlock(french, running)).toBeNull();
    expect(answerForCopy(french, "fr-FR", running)).not.toContain(french("chat.explanationTitle"));
  });

  it("says the explanation could not be written, and the figure stays", () => {
    const failed = entry({
      calculation,
      explanation: settleExplanation({ text: "", unavailable: true, unverifiedNumbers: [], modelArithmetic: [] }),
    });
    expect(explanationBlock(french, failed)).toContain(french("chat.explanationUnavailable"));
    expect(answerForCopy(english, "en-US", failed)).toContain("8 412,50 EUR");
  });

  it("reports a figure or a calculation the model added, as a caution read aloud", () => {
    const flagged = entry({
      calculation,
      explanation: settleExplanation({
        text: "Total 7777.",
        unavailable: false,
        unverifiedNumbers: ["7777"],
        modelArithmetic: ["10 + 40 = 50"],
      }),
    });
    expect(annotationLines(french, "fr-FR", flagged).some((line) => line.includes("« 7777 »"))).toBe(true);
    expect(cautionLines(french, "fr-FR", flagged).some((line) => line.includes("10 + 40 = 50"))).toBe(true);
  });
});

describe("the trace line of an explanation", () => {
  it("is informative, not a caution, and names the workbook", () => {
    const lines = workbookUseLines(french, [
      {
        file: "commandes.xlsx",
        how: "explained",
        reason: null,
        rowsSent: null,
        rowsTotal: null,
        truncated: false,
        count: null,
        candidates: [],
      },
    ]);
    expect(lines).toHaveLength(1);
    expect(lines[0]?.caution).toBe(false);
    expect(lines[0]?.text).toContain("commandes.xlsx");
    expect(lines[0]?.text).toContain("sans recevoir aucune ligne");
  });
});
