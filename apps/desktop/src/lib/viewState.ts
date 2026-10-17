import type { PreviewMode } from "./viewerFormats";

/**
 * What is remembered of a tab while it is not on screen, in memory only (nothing survives a
 * launch). The scroll position is a ratio of the scrollable height, so it stays right when the
 * zone is resized or the zoom changes.
 */
export interface TabViewState {
  /** 0 to 1 of the scrollable height, of the view the mode shows. */
  scrollRatio: number;
  mode: PreviewMode;
  /** Set only when the user chose a zoom: otherwise the document is fitted to the width as usual. */
  zoom: number | null;
  /** XLSX, ODS, CSV: the sheet shown, 0-based. */
  sheetIndex: number;
  /** PPTX, ODP: the slide shown, 0-based. */
  slideIndex: number;
}

export const FRESH_VIEW_STATE: TabViewState = {
  scrollRatio: 0,
  mode: "raw",
  zoom: null,
  sheetIndex: 0,
  slideIndex: 0,
};

/** A ratio kept inside 0 to 1; anything that is not a number is the top. */
export function clampRatio(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }
  return Math.min(1, Math.max(0, value));
}

/** The store's own copy of a state, with its ratio brought back into range. */
function sanitised(state: TabViewState): TabViewState {
  return { ...state, scrollRatio: clampRatio(state.scrollRatio) };
}

export interface ViewStateStore {
  /** Remembers the state of a tab, tied to the content (`sha256`) it was taken from. */
  save(id: string, sha256: string | null, state: TabViewState): void;
  /**
   * The state of a tab, or `undefined` when none was kept or when the file's content is another one
   * now. Reading does not remove it: React runs every effect twice in development, and a read that
   * consumed the entry would lose it on the second pass.
   */
  recall(id: string, sha256: string | null): TabViewState | undefined;
  clear(id: string): void;
  /** Keeps only the states of the tabs that are still open. */
  retain(ids: readonly string[]): void;
  clearAll(): void;
}

export function createViewStateStore(): ViewStateStore {
  const entries = new Map<string, { sha256: string | null; state: TabViewState }>();
  return {
    save(id, sha256, state) {
      entries.set(id, { sha256, state: sanitised(state) });
    },
    recall(id, sha256) {
      const entry = entries.get(id);
      return entry !== undefined && entry.sha256 === sha256 ? { ...entry.state } : undefined;
    },
    clear(id) {
      entries.delete(id);
    },
    retain(ids) {
      const keep = new Set(ids);
      for (const id of [...entries.keys()]) {
        if (!keep.has(id)) {
          entries.delete(id);
        }
      }
    },
    clearAll() {
      entries.clear();
    },
  };
}
