import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { WorkbookUse, WorkbookUseReason } from "./ipc";
import { foldedLines, workbookUseLines } from "./workbookUse";

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
