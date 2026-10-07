/**
 * How far from the end of the conversation still counts as being at the end. Sub-pixel rounding
 * leaves a couple of pixels behind on a container that has been scrolled all the way down, and a
 * zero tolerance would flicker the scroll-to-bottom button on and off while nothing moves.
 */
export const BOTTOM_TOLERANCE_PX = 24;

/** The three numbers a scroll container knows about itself, as the browser reports them. */
export function isNearBottom(
  scrollHeight: number,
  scrollTop: number,
  clientHeight: number,
): boolean {
  return scrollHeight - scrollTop - clientHeight <= BOTTOM_TOLERANCE_PX;
}

/** Gliding to the last message is an animation, so it obeys the system's reduced-motion setting. */
export function scrollBehaviour(prefersReducedMotion: boolean): ScrollBehavior {
  return prefersReducedMotion ? "auto" : "smooth";
}

/** False rather than a crash where `matchMedia` is missing, which is how a test environment looks. */
export function prefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;
}

/** How long after a wheel, touch, key or pointer action a scroll still counts as hers. */
export const USER_INTENT_WINDOW_MS = 600;

/** Keys that move a scroll container: pressing one is her choosing to read elsewhere. */
export const SCROLL_KEYS: readonly string[] = ["ArrowUp", "PageUp", "Home", "ArrowDown", "PageDown", "End", " "];

/**
 * Whether a streaming or just-finished answer keeps pulling the view to its end (HAP-1, BUG-08).
 *
 * A scroll event says the container is not at the end both when she scrolled up and when the content
 * grew past the tolerance a moment before the view followed it (a deterministic answer arriving as
 * one block, the footer lines and buttons a model answer gains after its text). Reading "not at the
 * end" as "she left" cut answers off. So the view stops following only for a scroll she caused, and
 * starts following again whenever she is back at the end.
 */
export function nextFollowing(previous: boolean, atEnd: boolean, causedByHer: boolean): boolean {
  if (atEnd) {
    return true;
  }
  return causedByHer ? false : previous;
}
