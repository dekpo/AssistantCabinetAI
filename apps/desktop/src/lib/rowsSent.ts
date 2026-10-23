import type { Translator } from "../i18n/translate";
import type { RowsEvidence } from "./ipc";
import { displayPathOfKey } from "./indexKey";
import { counted } from "./plural";
import { describeFilters } from "./tabularAnswer";

/**
 * The rows of the user's tables that the program chose and sent to the AI with a question (TABULAR step
 * 10, owner decision of 20 October 2026). Said under the answer in one line per workbook, listed in
 * their own disclosure exactly as they were written into the request, and copied only with the sources.
 * Nothing here is hidden: what the model read is what the user can open.
 */

/** "factures.csv : 15 lignes sur 15 transmises à l'IA." — the count of rows chosen out of those found. */
export function rowsSentLine(t: Translator, item: RowsEvidence): string {
  return t(item.truncated ? "chat.rowsSentSample" : "chat.rowsSent", {
    path: displayPathOfKey("data", item.file),
    rows: counted(item.shown, t("chat.rowOne"), t("chat.rowMany")),
    total: String(item.totalMatching),
  });
}

/** The workbook and its sheet, as the heading of the disclosure. */
export function rowsSentHeading(t: Translator, item: RowsEvidence): string {
  return t("chat.rowsSentSheet", { path: displayPathOfKey("data", item.file), sheet: item.sheet });
}

/** The filters that chose the rows, or null when the whole sheet was sent. */
export function rowsSentFilters(t: Translator, locale: string, item: RowsEvidence): string | null {
  return item.filters.length === 0
    ? null
    : t("chat.rowsSentFilters", { filters: describeFilters(t, item.filters, locale) });
}

/** The totals the engine computed over the rows sent, or null when the sheet has no amount to add. */
export function rowsSentTotals(t: Translator, locale: string, item: RowsEvidence): string | null {
  if (item.totalsOverShown.length === 0) {
    return null;
  }
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: 2 });
  return t("chat.rowsSentTotals", {
    totals: item.totalsOverShown
      .map((total) =>
        t("chat.rowsSentTotal", {
          column: `${total.letter} ${total.name}`,
          sum: total.unit === null ? number.format(total.sum) : `${number.format(total.sum)} ${total.unit}`,
        }),
      )
      .join(" ; "),
  });
}

/** The lines of every workbook's rows as plain text (what "copy with sources" puts on the clipboard):
 * the heading, the filters, the columns and the rows, tab-separated, and the totals. */
export function rowsSentCopy(t: Translator, locale: string, items: readonly RowsEvidence[]): string[] {
  const lines: string[] = [];
  for (const item of items) {
    lines.push(t("chat.rowsSentToggle") + " — " + rowsSentHeading(t, item));
    lines.push(rowsSentLine(t, item));
    const filters = rowsSentFilters(t, locale, item);
    if (filters !== null) {
      lines.push(filters);
    }
    lines.push([t("chat.rowsSentRow"), ...item.columns.map((column) => `${column.letter} ${column.name}`)].join("\t"));
    for (const row of item.rows) {
      lines.push([String(row.number), ...row.cells].join("\t"));
    }
    const totals = rowsSentTotals(t, locale, item);
    if (totals !== null) {
      lines.push(totals);
    }
  }
  return lines;
}
