import { useState, type ReactNode } from "react";
import type { ExplorerEntry } from "../lib/explorerModel";
import { shownRows, visibleWindow } from "../lib/explorerWindow";

/**
 * One family's list: flat, bounded, with its own scroll bar. The height is fixed by the budget of
 * rows (`lib/explorerWindow.ts`), never by the folder, and only the rows in view are drawn: the rest
 * is empty space of the right height, so a folder of a thousand files costs a few dozen rows. The
 * scroll area can take the keyboard focus, so the arrow keys scroll it.
 */
export function ExplorerList({
  entries,
  rows,
  rowHeight,
  label,
  renderRow,
}: {
  entries: readonly ExplorerEntry[];
  /** The most rows this list may show. */
  rows: number;
  rowHeight: number;
  /** What a screen reader calls the list. */
  label: string;
  renderRow: (entry: ExplorerEntry) => ReactNode;
}) {
  const [scrollTop, setScrollTop] = useState(0);
  const shown = shownRows(rows, entries.length);
  const viewport = shown * rowHeight;
  const slice = visibleWindow(entries.length, rowHeight, scrollTop, viewport);

  return (
    <div
      className="explorer-list"
      style={{ height: viewport }}
      tabIndex={entries.length > shown ? 0 : undefined}
      role="group"
      aria-label={label}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <ul
        className="explorer-list__rows"
        style={{ paddingTop: slice.before, paddingBottom: slice.after }}
      >
        {entries.slice(slice.first, slice.first + slice.count).map(renderRow)}
      </ul>
    </div>
  );
}
