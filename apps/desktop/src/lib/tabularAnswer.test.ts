import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { TabularAnswer, TabularRow } from "./ipc";
import {
  formatTabularAnswer,
  MAX_ROWS_SHOWN,
  tabularSourceLine,
  tabularSourceOf,
} from "./tabularAnswer";

/**
 * The sentence for an answer the tabular engine gave. Facts and machine codes in, her language
 * out, and never a sheet, column or value the engine did not return.
 */

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function sum(value: number): TabularAnswer {
  return {
    kind: "value",
    file: "factures.csv",
    value: { kind: "sum", value },
    locator: { sheet: "Facturation", headerRow: 0, column: "montant", rowRange: [0, 11] },
    derivation: { kind: "computed", operation: "sum", row_count: 12 },
  };
}

const nudge: TabularAnswer = {
  kind: "nudge",
  file: "factures.csv",
  reason: null,
  availableSheets: ["factures"],
  availableColumns: ["nom", "montant", "date"],
  exampleColumn: "montant",
  exampleGroup: "nom",
  filterColumn: null,
  filterValue: null,
};

describe("a computed value", () => {
  it("is printed exactly, in the way her language writes numbers", () => {
    const text = formatTabularAnswer(french, sum(1234.5), "fr-FR");

    expect(text).toContain("montant");
    // French groups thousands with a narrow no-break space and uses a decimal comma.
    expect(text).toMatch(/1\s234,5/u);
    expect(text).toContain("12 lignes");
  });

  it("keeps every significant digit it was given", () => {
    expect(formatTabularAnswer(english, sum(0.125), "en-US")).toContain("0.125");
  });
});

describe("the citation of a tabular answer", () => {
  it("names the sheet and the column, never a page", () => {
    const source = tabularSourceOf(sum(1));
    expect(source).not.toBeNull();

    for (const t of [english, french]) {
      const line = tabularSourceLine(t, source!);
      expect(line).toContain("factures.csv");
      expect(line).toContain("Facturation");
      expect(line).toContain("montant");
      expect(line.toLowerCase()).not.toContain("page");
    }
    expect(tabularSourceLine(french, source!)).toBe(
      "factures.csv, feuille Facturation, colonne montant",
    );
  });

  it("cites the sheet a structural fact was read from", () => {
    const source = tabularSourceOf({
      kind: "structural",
      file: "classeur.xlsx",
      answer: { kind: "row_count", sheet: "Stock", rows: 40 },
    });

    expect(source).toStrictEqual({ file: "classeur.xlsx", sheet: "Stock", column: null });
    expect(tabularSourceLine(french, source!)).toBe("classeur.xlsx, feuille Stock");
  });

  it("cites nothing for an answer that rests on no one sheet", () => {
    expect(tabularSourceOf(nudge)).toBeNull();
    expect(
      tabularSourceOf({
        kind: "structural",
        file: "classeur.xlsx",
        answer: { kind: "sheet_names", sheets: ["A", "B"] },
      }),
    ).toBeNull();
  });
});

describe("the nudge", () => {
  it("names the workbook's real columns and a concrete question built from one", () => {
    const text = formatTabularAnswer(french, nudge, "fr-FR");

    expect(text).toContain("factures.csv");
    expect(text).toContain("nom, montant, date");
    expect(text).toContain("somme de montant");
    expect(text).toContain("Quel nom a le plus de montant");
    expect(text).not.toContain("not_recognised");
  });

  it("says why when the engine refused, in words, not as a code", () => {
    const text = formatTabularAnswer(
      english,
      { ...nudge, reason: "formula_cannot_be_verified" } as TabularAnswer,
      "en-US",
    );

    expect(text).toContain("formulas");
    expect(text).not.toContain("formula_cannot_be_verified");
    expect(text).toContain("nom, montant, date");
  });

  it("lists the sheets when no sheet could be settled", () => {
    const text = formatTabularAnswer(
      french,
      {
        ...nudge,
        reason: "ambiguous_sheet_name",
        availableSheets: ["Consultations", "Facturation"],
        availableColumns: [],
        exampleColumn: null,
        exampleGroup: null,
      } as TabularAnswer,
      "fr-FR",
    );

    expect(text).toContain("Consultations, Facturation");
    expect(text).not.toContain("Par exemple");
  });

  it("is more direct the second time in a row", () => {
    const first = formatTabularAnswer(english, nudge, "en-US");
    const second = formatTabularAnswer(english, nudge, "en-US", true);

    expect(second.startsWith(first)).toBe(true);
    expect(second).toContain("previous question");
  });

  it("names the filter it saw and the column it was found in, never a code", () => {
    const text = formatTabularAnswer(
      english,
      {
        ...nudge,
        reason: "filter_not_supported",
        filterColumn: "fournisseur",
        filterValue: "Alpha",
      } as TabularAnswer,
      "en-US",
    );

    expect(text).toContain("Alpha");
    expect(text).toContain("fournisseur");
    expect(text).not.toContain("filter_not_supported");
  });

  it("omits the column line when none was found", () => {
    const text = formatTabularAnswer(
      english,
      {
        ...nudge,
        reason: "filter_not_supported",
        filterColumn: null,
        filterValue: "2021",
      } as TabularAnswer,
      "en-US",
    );

    expect(text).toContain("2021");
    expect(text).not.toContain("was found in the");
  });
});

describe("refusals and choices", () => {
  it("says plainly that a red workbook could not be read as a table", () => {
    expect(
      formatTabularAnswer(french, { kind: "workbook_unreadable", file: "notes.csv" }, "fr-FR"),
    ).toContain("notes.csv n'a pas pu être lu comme un tableau");
  });

  it("asks which workbook, listing only the ones selected", () => {
    const text = formatTabularAnswer(
      english,
      { kind: "which_workbook", candidates: ["achats.csv", "ventes.csv"] },
      "en-US",
    );

    expect(text).toContain("- achats.csv");
    expect(text).toContain("- ventes.csv");
  });
});

describe("a sorted listing", () => {
  function row(index: number, name: string): TabularRow {
    return {
      rowIndex: index,
      cells: [
        { column: "nom", text: name },
        { column: "montant", text: String(index) },
      ],
    };
  }

  it("is a table, with a pipe in a cell kept inside its cell", () => {
    const text = formatTabularAnswer(
      english,
      {
        kind: "value",
        file: "f.csv",
        value: { kind: "rows", value: [row(0, "A|B")] },
        locator: { sheet: "f", headerRow: 0, column: "montant", rowRange: [0, 0] },
        derivation: { kind: "computed", operation: "sort", row_count: 1 },
      },
      "en-US",
    );

    expect(text).toContain("| nom | montant |");
    expect(text).toContain("| A\\|B | 0 |");
  });

  it("shows the first rows and counts the rest", () => {
    const rows = Array.from({ length: MAX_ROWS_SHOWN + 3 }, (_, index) => row(index, `n${index}`));
    const text = formatTabularAnswer(
      french,
      {
        kind: "value",
        file: "f.csv",
        value: { kind: "rows", value: rows },
        locator: { sheet: "f", headerRow: 0, column: "montant", rowRange: [0, rows.length - 1] },
        derivation: { kind: "computed", operation: "sort", row_count: rows.length },
      },
      "fr-FR",
    );

    expect(text).toContain(`| n${MAX_ROWS_SHOWN - 1} |`);
    expect(text).not.toContain(`| n${MAX_ROWS_SHOWN} |`);
    expect(text).toContain("3 autres lignes");
  });
});

describe("which group costs the most", () => {
  function largest(top: { group: string; sum: number }[]): TabularAnswer {
    return {
      kind: "value",
      file: "revenue_sub_agency.csv",
      value: { kind: "largest_group", value: { groupColumn: "agency", top, groupCount: 12 } },
      locator: { sheet: "revenue_sub_agency", headerRow: 0, column: "amount", rowRange: [0, 99] },
      derivation: { kind: "computed", operation: "largest_group", row_count: 100 },
    };
  }

  it("names the leader and its total, says it is a total per group, and ranks the next ones", () => {
    const text = formatTabularAnswer(
      english,
      largest([
        { group: "Transit", sum: 8500.25 },
        { group: "Parks", sum: 2000.5 },
      ]),
      "en-US",
    );

    expect(text).toContain("Largest total amount by agency: **Transit**, with **8,500.25**.");
    expect(text).toContain("not the largest single row");
    expect(text).toContain("12 groups");
    expect(text).toContain("- Parks: 2,000.5");
    expect(text).toContain("100 rows");
  });

  it("calls a tie a tie rather than picking one", () => {
    const text = formatTabularAnswer(
      french,
      largest([
        { group: "Parks", sum: 10 },
        { group: "Water", sum: 10 },
        { group: "Health", sum: 3 },
      ]),
      "fr-FR",
    );

    expect(text).toContain("Égalité");
    expect(text).toContain("**Parks, Water**");
  });

  it("asks which column to add up, naming the candidates", () => {
    const text = formatTabularAnswer(
      french,
      { kind: "which_measure", file: "costs.csv", groupColumn: "agency", candidates: ["amount", "fee"] },
      "fr-FR",
    );

    expect(text).toContain("amount, fee");
    expect(text).toContain("« Quel agency a le plus de amount ? »");
  });
});

describe("a list of a column's values", () => {
  it("lists every value, and counts what does not fit", () => {
    const values = Array.from({ length: 53 }, (_, index) => `Agency ${index}`);
    const text = formatTabularAnswer(
      english,
      {
        kind: "value",
        file: "revenue_sub_agency.csv",
        value: { kind: "distinct", value: values },
        locator: { sheet: "revenue_sub_agency", headerRow: 0, column: "agency", rowRange: [0, 99] },
        derivation: { kind: "computed", operation: "distinct", row_count: 100 },
      },
      "en-US",
    );

    expect(text).toContain("The 53 values found in agency");
    expect(text).toContain("- Agency 0");
    expect(text).not.toContain("- Agency 50");
    expect(text).toContain("3 more values");
  });
});
