import type { Translator } from "../i18n/translate";
import type {
  AppliedFilter,
  NumericAggregate,
  StructuralAnswer,
  TabularAnswer,
  TabularDerivation,
  TabularRow,
  TabularValue,
} from "./ipc";
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
      return formatValue(t, answer.value, answer.locator, answer.derivation, locale);
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
    case "which_column":
      return t("tabularAnswer.whichColumn", {
        value: answer.value,
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

/** A weekday name (1 Monday .. 7 Sunday) in her language, from a fixed reference Monday - no
 * catalogue keys needed, `Intl` already knows every shipped locale's weekday names. */
function weekdayName(weekday: number, locale: string): string {
  const monday = new Date(Date.UTC(2024, 0, 1));
  const date = new Date(monday);
  date.setUTCDate(monday.getUTCDate() + (weekday - 1));
  return new Intl.DateTimeFormat(locale, { weekday: "long", timeZone: "UTC" }).format(date);
}

/** A month name (1 January .. 12 December) in her language, the same `Intl` approach. */
function monthName(month: number, locale: string): string {
  const date = new Date(Date.UTC(2024, month - 1, 1));
  return new Intl.DateTimeFormat(locale, { month: "long", timeZone: "UTC" }).format(date);
}

/** One applied filter, written as a short clause ("fournisseur = Alpha") for the "Understood as"
 * line - structured data in, a sentence in her language out, exactly like every other machine
 * code this module turns into prose (`docs/DECISIONS.md`, session 11). */
function describeFilter(t: Translator, filter: AppliedFilter, locale: string): string {
  const number = (amount: number) =>
    new Intl.NumberFormat(locale, { maximumFractionDigits: 6 }).format(amount);
  switch (filter.kind) {
    case "equals":
      return t("tabularAnswer.filterEquals", { column: filter.column, value: filter.value });
    case "contains":
      return t("tabularAnswer.filterContains", { column: filter.column, value: filter.value });
    case "greater_than":
      return t("tabularAnswer.filterGreaterThan", { column: filter.column, value: number(filter.threshold) });
    case "less_than":
      return t("tabularAnswer.filterLessThan", { column: filter.column, value: number(filter.threshold) });
    case "between":
      return t("tabularAnswer.filterBetween", {
        column: filter.column,
        low: number(filter.low),
        high: number(filter.high),
      });
    case "weekday":
      return t("tabularAnswer.filterWeekday", {
        column: filter.column,
        value: weekdayName(filter.weekday, locale),
      });
    case "month":
      return t("tabularAnswer.filterMonth", { column: filter.column, value: monthName(filter.month, locale) });
    case "year":
      return t("tabularAnswer.filterYear", { column: filter.column, value: String(filter.year) });
    case "date_range":
      // One day is written "date = 2026-01-23", not as a range that starts and ends the same day.
      return filter.start === filter.end
        ? t("tabularAnswer.filterEquals", { column: filter.column, value: filter.start })
        : t("tabularAnswer.filterDateRange", { column: filter.column, start: filter.start, end: filter.end });
    case "in":
      return t("tabularAnswer.filterIn", { column: filter.column, value: filter.values.join(", ") });
  }
}

function describeFilters(t: Translator, filters: AppliedFilter[], locale: string): string {
  return filters.map((filter) => describeFilter(t, filter, locale)).join(t("tabularAnswer.filterAnd"));
}

function formatValue(
  t: Translator,
  value: TabularValue,
  locator: { column: string | null; filters: AppliedFilter[] },
  derivation: TabularDerivation,
  locale: string,
): string {
  const column = locator.column;
  const number = (amount: number) =>
    new Intl.NumberFormat(locale, { maximumFractionDigits: 6 }).format(amount);
  /** A sum/min/max's own value, formatted in her language and with its unit beside it, exactly
   * as the column carried it (`docs/DECISIONS.md`, D2) - never translated, never repositioned. */
  const aggregate = (a: NumericAggregate) => number(a.value) + (a.unit === null ? "" : ` ${a.unit}`);
  /** "N cells could not be read as numbers", next to a sum/min/max whenever some were left out -
   * never silently dropped from the total (`docs/DECISIONS.md`, D2). */
  const unparsedNote = (a: NumericAggregate): string[] =>
    a.unparsed > 0
      ? [
          t("tabularAnswer.unparsedNote", {
            count: counted(a.unparsed, t("tabularAnswer.unparsedOne"), t("tabularAnswer.unparsedMany")),
          }),
        ]
      : [];
  const rows = counted(derivation.row_count, t("tabularAnswer.rowOne"), t("tabularAnswer.rowMany"));
  const over = t("tabularAnswer.computedOver", { rows });
  const named = column ?? "";

  const body = ((): string => {
  switch (value.kind) {
    case "count":
      return [t("tabularAnswer.count", { value: number(value.value) }), "", over].join("\n");
    case "sum":
      return [
        t("tabularAnswer.sum", { column: named, value: aggregate(value.value) }),
        "",
        over,
        ...unparsedNote(value.value),
      ].join("\n");
    case "min":
      return [
        t("tabularAnswer.min", { column: named, value: aggregate(value.value) }),
        "",
        over,
        ...unparsedNote(value.value),
      ].join("\n");
    case "max":
      return [
        t("tabularAnswer.max", { column: named, value: aggregate(value.value) }),
        "",
        over,
        ...unparsedNote(value.value),
      ].join("\n");
    case "mean":
      return [
        t("tabularAnswer.mean", { column: named, value: aggregate(value.value) }),
        "",
        over,
        ...unparsedNote(value.value),
      ].join("\n");
    case "median":
      return [
        t("tabularAnswer.median", { column: named, value: aggregate(value.value) }),
        "",
        over,
        ...unparsedNote(value.value),
      ].join("\n");
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
    case "least_group": {
      const { groupColumn, top, groupCount } = value.value;
      const best = top[0];
      if (best === undefined) {
        return over;
      }
      const leaders = top.filter((group) => group.sum === best.sum);
      const headline =
        leaders.length > 1
          ? t("tabularAnswer.leastGroupTie", {
              column: named,
              group: groupColumn,
              leaders: leaders.map((group) => group.group).join(", "),
              value: number(best.sum),
            })
          : t("tabularAnswer.leastGroup", {
              column: named,
              group: groupColumn,
              leader: best.group,
              value: number(best.sum),
            });
      /* Said every time: "which agency costs the least" is a total per agency, and a reader who
         expected the smallest single payment must not mistake one for the other. */
      const lines = [headline, "", t("tabularAnswer.groupTotalNoteLeast", { group: groupColumn })];
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
    case "top_groups": {
      const { groupColumn, top, groupCount, requested, capped } = value.value;
      const lines = [
        t("tabularAnswer.topGroups", {
          column: named,
          group: groupColumn,
          count: counted(top.length, t("tabularAnswer.groupOne"), t("tabularAnswer.groupMany")),
        }),
        "",
        ...top.map((group) => `- ${t("tabularAnswer.pair", { name: group.group, value: number(group.sum) })}`),
      ];
      if (capped) {
        lines.push("", t("tabularAnswer.topGroupsCapped", { requested: String(requested) }));
      }
      lines.push(
        "",
        t("tabularAnswer.groupRanking", {
          groups: counted(groupCount, t("tabularAnswer.groupOne"), t("tabularAnswer.groupMany")),
        }),
      );
      lines.push("", over);
      return lines.join("\n");
    }
    case "count_per_group":
      return [
        t("tabularAnswer.countPerGroup", { group: named }),
        "",
        ...value.value
          .slice(0, MAX_ROWS_SHOWN)
          .map((item) => `- ${t("tabularAnswer.pair", { name: item.group, value: number(item.count) })}`),
        ...(value.value.length > MAX_ROWS_SHOWN
          ? ["", t("tabularAnswer.moreGroups", { count: value.value.length - MAX_ROWS_SHOWN })]
          : []),
        "",
        over,
      ].join("\n");
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
  })();

  // "What was understood": every filter the value was actually computed under, written from
  // structured data, never invented prose (`docs/DECISIONS.md`, session 11).
  const understood = locator.filters.length > 0
    ? [body, "", t("tabularAnswer.understoodAs", { filters: describeFilters(t, locator.filters, locale) })].join(
        "\n",
      )
    : body;
  // A filtered figure over no row at all is the shape of a misread filter (HAP-1, Q14): say that
  // the selection is empty rather than present a zero as a finding.
  const withFilters =
    locator.filters.length > 0 && derivation.row_count === 0
      ? [understood, "", t("tabularAnswer.noRowMatched")].join("\n")
      : understood;

  // Session 14's hidden interpreter (`docs/SESSION-DATA-14-Query-Plan.md`): a value from a plan
  // the classifier itself did not write is already shown as a model answer, not a deterministic
  // one - `useChat.ts` gives it the same "Generated by {model} in {duration}" line any other
  // model-touched answer gets, which already names the model and marks it distinct, so this body
  // only adds the one thing that line cannot say on its own: it was the *question* the model
  // understood, never the number, and she can rephrase if it understood it wrong.
  return derivation.kind === "interpreted_by_model"
    ? [withFilters, "", t("tabularAnswer.interpretedRephrase")].join("\n")
    : withFilters;
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
  if (
    (nudge.reason === "non_numeric_column" || nudge.reason === "formula_cannot_be_verified") &&
    nudge.filterColumn !== null
  ) {
    lines.push(t("tabularAnswer.nudgeRefusedColumn", { column: nudge.filterColumn }));
  }
  if (nudge.reason === "value_not_found" && nudge.closeValues.length > 0) {
    lines.push(t("tabularAnswer.nudgeCloseValues", { values: nudge.closeValues.join(", ") }));
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
    // The advice depends on what went wrong: a value that matches nothing is fixed by spelling it as
    // the file does, not by naming a column (HAP-1, BUG-13).
    lines.push(t(nudge.reason === "value_not_found" ? "tabularAnswer.nudgeAgainValue" : "tabularAnswer.nudgeAgain"));
  }
  return lines.join("\n\n");
}
