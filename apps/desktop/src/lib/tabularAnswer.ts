import type { Translator } from "../i18n/translate";
import type { StructuralAnswer, TabularAnswer, TabularRow, TabularValue } from "./ipc";
import { counted } from "./plural";

/**
 * The sentence for an answer the tabular engine gave, with no model involved (tier 2,
 * `docs/SELECTION-AND-MEMORY.md`).
 *
 * Rust sends facts and machine codes and never prose, so the wording lives here, in her language
 * (`docs/LANGUAGE-AND-LOCALE.md`). Markdown, because the chat renders it and a sorted listing
 * reads far better as a table than as a sentence. Every sheet, column and value printed here came
 * from the engine: nothing is invented, and a number is shown exactly as computed, only formatted
 * the way her language writes numbers.
 */

/** Rows shown from a sort or a filter before the rest is counted rather than listed. */
export const MAX_ROWS_SHOWN = 50;

/** Where inside a workbook an answer came from: the tabular sibling of a document's page. */
export interface TabularSource {
  file: string;
  sheet: string;
  column: string | null;
}

export function formatTabularAnswer(
  t: Translator,
  answer: TabularAnswer,
  locale: string,
  /** The previous answer in this conversation was also a nudge: say so, and be more direct. */
  nudgedBefore = false,
): string {
  switch (answer.kind) {
    case "value":
      return formatValue(t, answer.value, answer.locator.column, answer.derivation, locale);
    case "structural":
      return formatStructural(t, answer.answer);
    case "nudge":
      return formatNudge(t, answer, nudgedBefore);
    case "which_measure":
      return t("tabularAnswer.whichMeasure", {
        file: answer.file,
        group: answer.groupColumn,
        columns: answer.candidates.join(", "),
        first: answer.candidates[0] ?? "",
      });
    case "workbook_unreadable":
      return t("tabularAnswer.unreadable", { file: answer.file });
    case "workbook_not_analysed":
      return t("tabularAnswer.notAnalysed", { file: answer.file });
    case "which_workbook":
      return [t("tabularAnswer.whichWorkbook"), "", ...answer.candidates.map((file) => `- ${file}`)].join(
        "\n",
      );
    case "ambiguous_reference":
      return [
        t("tabularAnswer.ambiguous", { query: answer.query }),
        "",
        ...answer.candidates.map((file) => `- ${file}`),
      ].join("\n");
    case "file_not_selected":
      return t("tabularAnswer.notSelected", { query: answer.query });
    case "no_matching_file":
      return t("tabularAnswer.noMatch", { query: answer.query });
    case "no_usable_table":
      return t("tabularAnswer.noUsableTable");
  }
}

/** The file and sheet an answer rests on, when it rests on one sheet; `null` otherwise (a list of
 * sheets, a nudge, a refusal). Shown where a document answer lists its pages. */
export function tabularSourceOf(answer: TabularAnswer): TabularSource | null {
  if (answer.kind === "value") {
    return { file: answer.file, sheet: answer.locator.sheet, column: answer.locator.column };
  }
  if (answer.kind === "structural" && answer.answer.kind !== "sheet_names") {
    const column = answer.answer.kind === "is_column_numeric" ? answer.answer.column : null;
    return { file: answer.file, sheet: answer.answer.sheet, column };
  }
  return null;
}

/** "factures.csv, feuille Facturation, colonne montant": where a document source says "page 3". */
export function tabularSourceLine(t: Translator, source: TabularSource): string {
  return source.column === null
    ? t("chat.sourceItemSheet", { path: source.file, sheet: source.sheet })
    : t("chat.sourceItemSheetColumn", {
        path: source.file,
        sheet: source.sheet,
        column: source.column,
      });
}

function formatValue(
  t: Translator,
  value: TabularValue,
  column: string | null,
  derivation: { operation: string; row_count: number },
  locale: string,
): string {
  const number = (amount: number) =>
    new Intl.NumberFormat(locale, { maximumFractionDigits: 6 }).format(amount);
  const rows = counted(derivation.row_count, t("tabularAnswer.rowOne"), t("tabularAnswer.rowMany"));
  const over = t("tabularAnswer.computedOver", { rows });
  const named = column ?? "";

  switch (value.kind) {
    case "count":
      return [t("tabularAnswer.count", { value: number(value.value) }), "", over].join("\n");
    case "sum":
      return [t("tabularAnswer.sum", { column: named, value: number(value.value) }), "", over].join("\n");
    case "min":
      return [t("tabularAnswer.min", { column: named, value: number(value.value) }), "", over].join("\n");
    case "max":
      return [t("tabularAnswer.max", { column: named, value: number(value.value) }), "", over].join("\n");
    case "distinct":
      return [
        t("tabularAnswer.distinct", {
          column: named,
          count: counted(value.value.length, t("tabularAnswer.valueOne"), t("tabularAnswer.valueMany")),
        }),
        "",
        ...value.value.slice(0, MAX_ROWS_SHOWN).map((item) => `- ${item}`),
        ...(value.value.length > MAX_ROWS_SHOWN
          ? ["", t("tabularAnswer.moreValues", { count: value.value.length - MAX_ROWS_SHOWN })]
          : []),
      ].join("\n");
    case "group_sums": {
      /* Largest first, the first `MAX_ROWS_SHOWN` listed: forty agencies read as a ranking, not
         as an alphabetical dump. */
      const ordered = [...value.value].sort((a, b) => b.sum - a.sum);
      return [
        t("tabularAnswer.groupSums", { column: named }),
        "",
        ...ordered
          .slice(0, MAX_ROWS_SHOWN)
          .map((group) => `- ${t("tabularAnswer.pair", { name: group.group, value: number(group.sum) })}`),
        ...(ordered.length > MAX_ROWS_SHOWN
          ? ["", t("tabularAnswer.moreGroups", { count: ordered.length - MAX_ROWS_SHOWN })]
          : []),
        "",
        over,
      ].join("\n");
    }
    case "largest_group": {
      const { groupColumn, top, groupCount } = value.value;
      const best = top[0];
      if (best === undefined) {
        return over;
      }
      const leaders = top.filter((group) => group.sum === best.sum);
      const headline =
        leaders.length > 1
          ? t("tabularAnswer.largestGroupTie", {
              column: named,
              group: groupColumn,
              leaders: leaders.map((group) => group.group).join(", "),
              value: number(best.sum),
            })
          : t("tabularAnswer.largestGroup", {
              column: named,
              group: groupColumn,
              leader: best.group,
              value: number(best.sum),
            });
      /* Said every time: "which agency costs the most" is a total per agency, and a reader who
         expected the largest single payment must not mistake one for the other. */
      const lines = [headline, "", t("tabularAnswer.groupTotalNote", { group: groupColumn })];
      if (top.length > 1) {
        lines.push(
          "",
          t("tabularAnswer.groupRanking", {
            groups: counted(groupCount, t("tabularAnswer.groupOne"), t("tabularAnswer.groupMany")),
          }),
          "",
          ...top.map(
            (group) => `- ${t("tabularAnswer.pair", { name: group.group, value: number(group.sum) })}`,
          ),
        );
      }
      lines.push("", over);
      return lines.join("\n");
    }
    case "largest_row":
      return [t("tabularAnswer.largestRow", { column: named }), "", ...rowLines(t, value.value)].join(
        "\n",
      );
    case "row":
      return [t("tabularAnswer.row", { row: value.value.rowIndex + 1 }), "", ...rowLines(t, value.value)].join(
        "\n",
      );
    case "rows":
      return [
        t(derivation.operation === "sort" ? "tabularAnswer.sorted" : "tabularAnswer.filtered", {
          column: named,
          count: counted(value.value.length, t("tabularAnswer.rowOne"), t("tabularAnswer.rowMany")),
        }),
        "",
        ...rowsTable(t, value.value),
      ].join("\n");
  }
}

function rowLines(t: Translator, row: TabularRow): string[] {
  return row.cells.map((cell) => `- ${t("tabularAnswer.pair", { name: cell.column, value: cell.text })}`);
}

/** A Markdown table, first `MAX_ROWS_SHOWN` rows, the rest counted. A `|` inside a cell would
 * split it into two columns, so it is escaped. */
function rowsTable(t: Translator, rows: TabularRow[]): string[] {
  const first = rows[0];
  if (first === undefined) {
    return [];
  }
  const cell = (text: string) => text.replace(/\|/g, "\\|").replace(/\n/g, " ");
  const header = `| ${first.cells.map((each) => cell(each.column)).join(" | ")} |`;
  const rule = `| ${first.cells.map(() => "---").join(" | ")} |`;
  const body = rows
    .slice(0, MAX_ROWS_SHOWN)
    .map((row) => `| ${row.cells.map((each) => cell(each.text)).join(" | ")} |`);
  const rest =
    rows.length > MAX_ROWS_SHOWN
      ? ["", t("tabularAnswer.moreRows", { count: rows.length - MAX_ROWS_SHOWN })]
      : [];
  return [header, rule, ...body, ...rest];
}

function formatStructural(t: Translator, answer: StructuralAnswer): string {
  switch (answer.kind) {
    case "sheet_names":
      return t("tabularAnswer.sheetNames", {
        count: counted(answer.sheets.length, t("tabularAnswer.sheetOne"), t("tabularAnswer.sheetMany")),
        sheets: answer.sheets.join(", "),
      });
    case "row_count":
      return t("tabularAnswer.rowCount", {
        sheet: answer.sheet,
        rows: counted(answer.rows, t("tabularAnswer.rowOne"), t("tabularAnswer.rowMany")),
      });
    case "column_names":
      return t("tabularAnswer.columnNames", {
        sheet: answer.sheet,
        columns: answer.columns.join(", "),
      });
    case "is_column_numeric":
      return t(answer.numeric ? "tabularAnswer.numericYes" : "tabularAnswer.numericNo", {
        column: answer.column,
      });
    case "has_formulas":
      return t(answer.has_formulas ? "tabularAnswer.formulasYes" : "tabularAnswer.formulasNo", {
        sheet: answer.sheet,
      });
  }
}

/**
 * A question the engine could not answer exactly, turned into the next question to ask: what it
 * could not do, then the workbook's real columns (or sheets), then what the engine can compute,
 * with an example built from one of those real columns (`docs/SESSION-DATA-05-TABULAR-UI.md` 5b).
 */
function formatNudge(
  t: Translator,
  nudge: Extract<TabularAnswer, { kind: "nudge" }>,
  nudgedBefore: boolean,
): string {
  const lines = [
    t(`tabularAnswer.nudge.${nudge.reason ?? "not_recognised"}`, {
      file: nudge.file,
      value: nudge.filterValue ?? "",
    }),
  ];
  if (nudge.reason === "filter_not_supported" && nudge.filterColumn !== null) {
    lines.push(t("tabularAnswer.nudgeFilterColumn", { column: nudge.filterColumn }));
  }
  if (nudge.availableColumns.length > 0) {
    lines.push(t("tabularAnswer.nudgeColumns", { columns: nudge.availableColumns.join(", ") }));
  }
  if (nudge.availableSheets.length > 1 || nudge.availableColumns.length === 0) {
    lines.push(t("tabularAnswer.nudgeSheets", { sheets: nudge.availableSheets.join(", ") }));
  }
  lines.push(t("tabularAnswer.nudgeOperations"));
  if (nudge.exampleColumn !== null) {
    lines.push(t("tabularAnswer.nudgeExample", { column: nudge.exampleColumn }));
    if (nudge.exampleGroup !== null) {
      lines.push(
        t("tabularAnswer.nudgeExampleGroup", { column: nudge.exampleColumn, group: nudge.exampleGroup }),
      );
    }
  }
  if (nudgedBefore) {
    lines.push(t("tabularAnswer.nudgeAgain"));
  }
  return lines.join("\n\n");
}
