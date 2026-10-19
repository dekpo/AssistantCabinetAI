import type { ExplorerDensity } from "./ipc";

/*
 * The arithmetic of the explorer's lists, with no DOM: how tall a row is, how many rows each open
 * list may show, and which rows of a long list are drawn. A list of a thousand files draws a few
 * dozen rows, and the box never grows with the folder. Rows have a fixed height, so no measuring and
 * no dependency are needed (`EXPLORER-02-view.md`, "Bounded lists, folding, windowing").
 */

/** The height of one row, in px: `compact` is the default, `comfortable` is a setting. */
export const ROW_HEIGHT: Readonly<Record<ExplorerDensity, number>> = {
  compact: 24,
  comfortable: 32,
};

/** Rows drawn above and below the visible ones, so a fast scroll does not show blank space. */
export const OVERSCAN_ROWS = 4;

/** Which family lists are open. */
export interface OpenLists {
  documents: boolean;
  tables: boolean;
}

/** The rows each list may show at most. */
export interface RowBudget {
  documents: number;
  tables: number;
}

/** Rows shared by the open lists: both open, six each; one open, twelve; none open, the headers
 * alone. Owner to confirm while building (`EXPLORER-00-PLAN.md`, section 2, point 10). */
export const ROWS_BOTH_OPEN = 6;
export const ROWS_ONE_OPEN = 12;

export function rowBudget(open: OpenLists): RowBudget {
  const openCount = Number(open.documents) + Number(open.tables);
  const rows = openCount === 2 ? ROWS_BOTH_OPEN : openCount === 1 ? ROWS_ONE_OPEN : 0;
  return { documents: open.documents ? rows : 0, tables: open.tables ? rows : 0 };
}

/** The rows a list of `total` files really shows: a short list is not padded up to the budget. */
export function shownRows(budget: number, total: number): number {
  return Math.max(0, Math.min(budget, total));
}

/** Which rows are drawn, and the empty space that stands in for the others. */
export interface WindowSlice {
  /** Index of the first drawn row. */
  first: number;
  /** How many rows are drawn. */
  count: number;
  /** Height, in px, of the rows before the first drawn one. */
  before: number;
  /** Height, in px, of the rows after the last drawn one. */
  after: number;
}

/**
 * The rows to draw for a list of `total` rows of `rowHeight` px, in a viewport of `viewport` px
 * scrolled by `scrollTop` px. Any position is accepted, including a stale one past the end (after
 * the list shrank): it is clamped, so the slice always lies inside the list.
 */
export function visibleWindow(
  total: number,
  rowHeight: number,
  scrollTop: number,
  viewport: number,
  overscan: number = OVERSCAN_ROWS,
): WindowSlice {
  if (total <= 0 || rowHeight <= 0) {
    return { first: 0, count: 0, before: 0, after: 0 };
  }
  const contentHeight = total * rowHeight;
  const maxScroll = Math.max(0, contentHeight - Math.max(0, viewport));
  const top = Math.min(Math.max(0, scrollTop), maxScroll);
  const firstVisible = Math.floor(top / rowHeight);
  const lastVisible = Math.min(total - 1, Math.ceil((top + Math.max(0, viewport)) / rowHeight) - 1);
  const first = Math.max(0, firstVisible - overscan);
  const last = Math.min(total - 1, Math.max(firstVisible, lastVisible) + overscan);
  const count = last - first + 1;
  return {
    first,
    count,
    before: first * rowHeight,
    after: (total - first - count) * rowHeight,
  };
}
