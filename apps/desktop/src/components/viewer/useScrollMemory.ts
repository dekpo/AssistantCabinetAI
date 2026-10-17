import { useEffect, useRef, type RefObject } from "react";
import {
  RESTORE_START,
  restoreStep,
  scrollRange,
  scrollRatio,
  type RestoreProgress,
} from "../../lib/viewerScroll";

/** What a view needs to take part in the memory of its tab. */
export interface ScrollMemory {
  /** Where to put the view when it is ready, 0 to 1 of its scrollable height. */
  ratio: number;
  /** The view was scrolled by the user (never while a restore is running). */
  onRatio: (ratio: number) => void;
  /** The restore ran to its end (or the user took over): only now may the state be saved. */
  onRestored: () => void;
}

/**
 * The element inside `root` (or `root` itself) that scrolls the most. Looked for by what the
 * browser says about it (it overflows and its style lets it scroll), never by a class of the
 * library, so a change of the library's markup does not break it. The cheap test comes first, so
 * the style of an element is read only when it really overflows.
 */
export function findScroller(root: HTMLElement): HTMLElement | null {
  let best: HTMLElement | null = null;
  let bestRange = 0;
  const consider = (element: HTMLElement) => {
    const range = scrollRange(element);
    if (range <= bestRange) {
      return;
    }
    const overflow = getComputedStyle(element).overflowY;
    if (overflow === "auto" || overflow === "scroll" || overflow === "overlay") {
      best = element;
      bestRange = range;
    }
  };
  consider(root);
  root.querySelectorAll<HTMLElement>("*").forEach(consider);
  return best;
}

/** Input that means the user is in charge of the position: the restore stops fighting her. */
const USER_INPUT = ["wheel", "keydown", "pointerdown", "touchstart"] as const;

type Phase = "waiting" | "restoring" | "live";

/**
 * Remembers where a scrolling view is, and puts it back. `target` is the element whose subtree
 * scrolls (scroll events do not bubble, so they are listened to on the way down). Three phases:
 * - `waiting` until `active` (the document is drawn): scrolls of the library's own start-up are not
 *   the user's and are not recorded;
 * - `restoring`: the position is set again frame after frame while the document is laid out (see
 *   `restoreStep`), bounded in time, and abandoned at the user's first wheel, key or pointer input;
 * - `live`: every scroll is recorded through `onRatio`.
 * Nothing is recorded outside `live`, and `onRestored` fires only when the restore ended, which is
 * what keeps a view that was destroyed half way (React runs every effect twice in development)
 * from saving a position it never reached.
 */
export function useScrollMemory(
  target: RefObject<HTMLElement | null>,
  active: boolean,
  memory: ScrollMemory,
  find: (root: HTMLElement) => HTMLElement | null = findScroller,
): void {
  const phase = useRef<Phase>("waiting");
  const scroller = useRef<HTMLElement | null>(null);
  const latest = useRef(memory);
  latest.current = memory;

  useEffect(() => {
    const root = target.current;
    if (root === null) {
      return;
    }
    const onScroll = (event: Event) => {
      const element = event.target;
      if (phase.current !== "live" || !(element instanceof HTMLElement)) {
        return;
      }
      const range = scrollRange(element);
      const current = scroller.current;
      if (range === 0) {
        return;
      }
      if (current === null || !current.isConnected || range > scrollRange(current)) {
        scroller.current = element;
      }
      if (element === scroller.current) {
        latest.current.onRatio(scrollRatio(element.scrollTop, element));
      }
    };
    root.addEventListener("scroll", onScroll, true);
    return () => root.removeEventListener("scroll", onScroll, true);
  }, [target]);

  useEffect(() => {
    const root = target.current;
    if (!active || root === null) {
      return;
    }
    const wanted = latest.current.ratio;
    const startedAt = performance.now();
    let progress: RestoreProgress = RESTORE_START;
    let frame = 0;
    scroller.current = null;

    const finish = () => {
      for (const type of USER_INPUT) {
        root.removeEventListener(type, finish, true);
      }
      cancelAnimationFrame(frame);
      const element = scroller.current;
      if (element !== null && element.isConnected && wanted > 0) {
        latest.current.onRatio(scrollRatio(element.scrollTop, element));
      }
      phase.current = "live";
      latest.current.onRestored();
    };
    const tick = () => {
      const current = scroller.current;
      const element = current !== null && current.isConnected ? current : find(root);
      scroller.current = element;
      const step = restoreStep(
        wanted,
        element ?? { scrollHeight: 0, clientHeight: 0 },
        progress,
        performance.now() - startedAt,
      );
      progress = step.progress;
      if (element !== null && step.scrollTop !== null) {
        element.scrollTop = step.scrollTop;
      }
      if (step.done) {
        finish();
      } else {
        frame = requestAnimationFrame(tick);
      }
    };

    if (wanted <= 0) {
      // The top needs nothing set.
      phase.current = "live";
      latest.current.onRestored();
    } else {
      phase.current = "restoring";
      for (const type of USER_INPUT) {
        root.addEventListener(type, finish, true);
      }
      frame = requestAnimationFrame(tick);
    }
    return () => {
      for (const type of USER_INPUT) {
        root.removeEventListener(type, finish, true);
      }
      cancelAnimationFrame(frame);
      phase.current = "waiting";
    };
  }, [active, target, find]);
}
