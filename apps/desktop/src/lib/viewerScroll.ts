import { clampRatio } from "./viewState";

/** How long a restore keeps trying while the document is still being laid out. */
export const RESTORE_LIMIT_MS = 2000;
/** Frames in a row with the same scrollable height before the layout counts as settled. */
export const STABLE_FRAMES = 3;

/** The size of a scrolling box, as the DOM reports it. */
export interface ScrollBox {
  scrollHeight: number;
  clientHeight: number;
}

/** How far the box can scroll, never negative. */
export function scrollRange(box: ScrollBox): number {
  return Math.max(0, box.scrollHeight - box.clientHeight);
}

/** `scrollTop` as a 0 to 1 ratio of the scrollable height; a box that does not scroll is 0. */
export function scrollRatio(scrollTop: number, box: ScrollBox): number {
  const range = scrollRange(box);
  return range === 0 ? 0 : clampRatio(scrollTop / range);
}

/** The `scrollTop` that puts the box at `ratio` of its scrollable height. */
export function scrollTopFor(ratio: number, box: ScrollBox): number {
  return Math.round(clampRatio(ratio) * scrollRange(box));
}

/** What a restore remembers between two frames. */
export interface RestoreProgress {
  lastHeight: number | null;
  stableFrames: number;
}

export const RESTORE_START: RestoreProgress = { lastHeight: null, stableFrames: 0 };

export interface RestoreStep {
  /** Where to put the box now; `null` while it cannot scroll yet. */
  scrollTop: number | null;
  progress: RestoreProgress;
  /** The layout settled, or the time ran out: stop trying. */
  done: boolean;
}

/**
 * One frame of a restore. A document that is drawn progressively keeps growing, so the position is
 * set again on every frame until the scrollable height has stopped changing for a few frames, and
 * the loop ends at the time limit whatever happens: it never waits forever.
 */
export function restoreStep(
  ratio: number,
  box: ScrollBox,
  progress: RestoreProgress,
  elapsedMs: number,
): RestoreStep {
  const scrollable = scrollRange(box) > 0;
  const stableFrames =
    scrollable && progress.lastHeight === box.scrollHeight ? progress.stableFrames + 1 : 0;
  return {
    scrollTop: scrollable ? scrollTopFor(ratio, box) : null,
    progress: { lastHeight: box.scrollHeight, stableFrames },
    done: stableFrames >= STABLE_FRAMES || elapsedMs >= RESTORE_LIMIT_MS,
  };
}
