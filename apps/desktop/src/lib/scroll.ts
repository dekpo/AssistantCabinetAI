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
