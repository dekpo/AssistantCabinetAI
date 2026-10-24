import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { LayoutNote, WorkbookUse, WorkbookUseReason } from "./ipc";
import { foldedLines, hiddenRowsOf, workbookUseLines } from "./workbookUse";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function use(extra: Partial<WorkbookUse>): WorkbookUse {
  return {
    file: "ventes.csv",
    how: "card_sent",
    reason: null,
    rowsSent: null,
    rowsTotal: null,
    rowsTruncated: false,
    truncated: false,
    count: null,
    candidates: [],
    ...extra,
  };
}

describe("one line per workbook", () => {
  it("says a named workbook gave the AI its structure, and an implied one says why", () => {
    const lines = workbookUseLines(french, [
      use({}),
      use({ reason: "implied_single" }),
      use({ reason: "implied_by_column" }),
    ]);

    expect(lines.map((line) => line.text)).toStrictEqual([
      "ventes.csv : cité dans votre question, l'IA en a reçu la structure.",
      "ventes.csv : seul tableur coché, l'IA en a reçu la structure.",
      "ventes.csv : votre question cite l'une de ses colonnes, l'IA en a reçu la structure.",
    ]);
    expect(lines.every((line) => !line.caution)).toBe(true);
  });

  it("does not repeat the rows the Sources line already counts", () => {
    const [line] = workbookUseLines(english, [use({ rowsSent: 12, rowsTotal: 480 })]);

    expect(line?.text).not.toContain("12");
    expect(line?.text).not.toContain("480");
  });

  it("shows the plain path of a workbook of the other folder", () => {
    const [line] = workbookUseLines(english, [use({ how: "computed", file: "/documents/Factures/2025.xlsx" })]);

    expect(line?.text).toBe("Factures/2025.xlsx: answer computed by the program, without the AI.");
  });

  it("warns when a card was shrunk", () => {
    const [line] = workbookUseLines(english, [use({ truncated: true })]);

    expect(line?.caution).toBe(true);
    expect(line?.text).toContain("too wide");
  });

  it("explains every refusal and says what to do, and counts what was left out", () => {
    const refusals: WorkbookUseReason[] = [
      "named_not_selected",
      "not_analysed",
      "unreadable",
      "changed_since_analysis",
      "ambiguous_name",
      "not_sent",
      "sheet_to_choose",
    ];
    const lines = workbookUseLines(
      french,
      refusals.map((reason) =>
        use({ how: "not_used", reason, candidates: ["2025/ventes.csv", "2026/ventes.csv"] }),
      ),
    );

    expect(lines.every((line) => line.caution)).toBe(true);
    expect(lines[0]?.text).toContain("cochez-le");
    expect(lines[1]?.text).toContain("Lancez l'analyse");
    expect(lines[4]?.text).toContain("2025/ventes.csv, 2026/ventes.csv");
    expect(lines[6]?.text).toContain("plusieurs feuilles");
    const [one, many] = workbookUseLines(french, [
      use({ file: "", how: "not_used", reason: "not_relevant", count: 1 }),
      use({ file: "", how: "not_used", reason: "not_relevant", count: 3 }),
    ]);
    expect(one?.text).toBe(
      "1 tableur coché n'a pas servi pour cette réponse : citez-le par son nom pour l'inclure.",
    );
    expect(many?.text).toContain("3 tableurs cochés n'ont pas servi");
    expect(one?.caution).toBe(false);
  });
});

describe("folding long lists", () => {
  const info = { text: "a", caution: false };
  const warn = { text: "b", caution: true };

  it("shows up to three lines at once", () => {
    expect(foldedLines([info, warn, info])).toStrictEqual({ shown: [info, warn, info], folded: [] });
  });

  it("folds the informative lines of a longer list and keeps the cautions in view", () => {
    expect(foldedLines([info, info, warn, info])).toStrictEqual({ shown: [warn], folded: [info, info, info] });
  });
});

function note(extra: Partial<LayoutNote>): LayoutNote {
  return {
    sheet: "Journal",
    hiddenRows: 0,
    filteredRows: 0,
    rowsLeftOut: false,
    totalRows: 0,
    mergedFilled: 0,
    blankRows: 0,
    noteRows: 0,
    tables: 1,
    hiddenSheets: 0,
    ...extra,
  };
}

describe("what the layout of the sheet did to a figure", () => {
  it("says hidden rows were counted, and cautions about it", () => {
    const lines = workbookUseLines(english, [
      use({ how: "computed", layout: note({ hiddenRows: 2 }) }),
    ]);

    expect(lines).toHaveLength(2);
    expect(lines[1]?.text).toBe("2 rows hidden in the file are counted in this result.");
    expect(lines[1]?.caution).toBe(true);
  });

  it("says they were left out when the answer left them out", () => {
    const lines = workbookUseLines(french, [
      use({ how: "computed", layout: note({ filteredRows: 1, rowsLeftOut: true }) }),
    ]);

    expect(lines[1]?.text).toBe("1 ligne écartée par un filtre du fichier n'est pas utilisée.");
  });

  it("states a total row, merged cells, notes and hidden sheets without a caution", () => {
    const lines = workbookUseLines(english, [
      use({
        how: "computed",
        layout: note({ totalRows: 1, mergedFilled: 2, noteRows: 1, hiddenSheets: 1 }),
      }),
    ]);

    expect(lines.slice(1).map((line) => line.text)).toStrictEqual([
      "1 total row at the foot of the table is not counted.",
      "2 merged cells were copied onto the rows they cover.",
      "1 note row around the table is not read.",
      "1 hidden sheet of the workbook is not read: name it to use it.",
    ]);
    expect(lines.slice(1).map((line) => line.caution)).toStrictEqual([true, false, false, false]);
  });

  it("adds nothing for a plain sheet", () => {
    expect(workbookUseLines(english, [use({ how: "computed", layout: null })])).toHaveLength(1);
    expect(workbookUseLines(english, [use({ how: "computed" })])).toHaveLength(1);
  });

  it("asks for a new analysis when the workbook was read by an older version", () => {
    const lines = workbookUseLines(english, [
      use({ how: "computed", layout: note({ unread: true }) }),
    ]);

    expect(lines[1]?.text).toContain("analysed by an older version");
    expect(lines[1]?.caution).toBe(true);
  });

  it("finds the hidden rows of the figure for the one click, and none otherwise", () => {
    expect(hiddenRowsOf([use({ how: "computed", layout: note({ hiddenRows: 2 }) })])).toStrictEqual({
      count: 2,
      leftOut: false,
    });
    expect(
      hiddenRowsOf([use({ how: "computed", layout: note({ filteredRows: 3, rowsLeftOut: true }) })]),
    ).toStrictEqual({ count: 3, leftOut: true });
    expect(hiddenRowsOf([use({ how: "computed", layout: note({ totalRows: 1 }) })])).toBeNull();
    expect(hiddenRowsOf(undefined)).toBeNull();
  });
});
