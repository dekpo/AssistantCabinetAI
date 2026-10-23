import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { ChatEntry } from "../state/useChat";
import { annotationLines, answerForCopy, cautionLines } from "./annotations";
import type { RowsEvidence } from "./ipc";
import { rowsSentCopy, rowsSentFilters, rowsSentHeading, rowsSentLine, rowsSentTotals } from "./rowsSent";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

/** What the engine chose for "write a mail to supplier Beta": two of his three invoices fit. */
function rows(extra: Partial<RowsEvidence> = {}): RowsEvidence {
  return {
    file: "factures.csv",
    sheet: "Feuille1",
    filters: [{ kind: "equals", column: "fournisseur", value: "Beta" }],
    columns: [
      { letter: "A", name: "fournisseur" },
      { letter: "B", name: "montant" },
    ],
    hiddenColumns: 0,
    rows: [
      { number: 4, cells: ["Beta", "1 100,50 €"] },
      { number: 8, cells: ["Beta", "1 500,50 €"] },
    ],
    totalMatching: 2,
    shown: 2,
    truncated: false,
    dataRows: 60,
    totalsOverShown: [{ letter: "B", name: "montant", sum: 2601, count: 2, unit: "€" }],
    ...extra,
  };
}

function entry(extra: Partial<ChatEntry>): ChatEntry {
  return { id: "a", role: "assistant", content: "Bonjour Beta.", createdAt: 0, ...extra };
}

describe("the rows the program sent to the AI", () => {
  it("are counted in one line per workbook, in her language", () => {
    expect(rowsSentLine(french, rows())).toBe("factures.csv : 2 lignes sur 2 transmises à l'IA.");
    expect(rowsSentLine(english, rows())).toBe("factures.csv: 2 rows of 2 sent to the AI.");
  });

  it("say when only a sample of the rows found was sent", () => {
    const sample = rows({ truncated: true, totalMatching: 15, shown: 6 });

    expect(rowsSentLine(french, sample)).toBe(
      "factures.csv : 6 lignes sur 15 transmises à l'IA (un échantillon des lignes trouvées, avec les chiffres de toutes les lignes trouvées).",
    );
  });

  it("name the workbook of the other folder by its plain path and the sheet", () => {
    const heading = rowsSentHeading(english, rows({ file: "/documents/Factures/2025.xlsx", sheet: "Mars" }));

    expect(heading).toBe("Factures/2025.xlsx, sheet “Mars”");
  });

  it("write the filters that chose them, and nothing when the whole sheet was sent", () => {
    expect(rowsSentFilters(english, "en-US", rows())).toBe("Filters: fournisseur = Beta");
    expect(rowsSentFilters(english, "en-US", rows({ filters: [] }))).toBeNull();
  });

  it("show the totals the engine computed over them, with the unit", () => {
    expect(rowsSentTotals(english, "en-US", rows())).toBe(
      "Totals computed by the program over these rows: B montant: 2,601 €",
    );
    expect(rowsSentTotals(english, "en-US", rows({ totalsOverShown: [] }))).toBeNull();
  });

  it("are listed exactly as sent, tab-separated, only when the sources are copied", () => {
    const withRows = entry({ mixedRowsSent: [rows()] });

    const copied = rowsSentCopy(english, "en-US", [rows()]);
    expect(copied).toContain("A fournisseur\tB montant".replace("A fournisseur", "Row\tA fournisseur"));
    expect(copied).toContain("4\tBeta\t1 100,50 €");
    expect(copied).toContain("8\tBeta\t1 500,50 €");

    // Off by default: the line under the answer is copied, the rows are not.
    expect(answerForCopy(english, "en-US", withRows)).toContain("2 rows of 2 sent to the AI");
    expect(answerForCopy(english, "en-US", withRows)).not.toContain("1 100,50");
    expect(answerForCopy(english, "en-US", withRows, true)).toContain("4\tBeta\t1 100,50 €");
  });

  it("are a line under the answer and a caution read aloud: data left for a model", () => {
    const withRows = entry({ mixedRowsSent: [rows()] });

    expect(annotationLines(english, "en-US", withRows)).toContain("factures.csv: 2 rows of 2 sent to the AI.");
    expect(cautionLines(english, "en-US", withRows)).toStrictEqual(["factures.csv: 2 rows of 2 sent to the AI."]);
    expect(annotationLines(english, "en-US", entry({}))).toStrictEqual([]);
  });
});
