/** How many lines of a long question stay on screen when it is collapsed. */
export const CLAMP_LINES = 4;

/**
 * A line-clamped element hides the rest of its text rather than scrolling it, so the only honest
 * way to know a toggle is needed is to compare what the content wants with what is shown. One
 * pixel of tolerance, because a fractional line height rounds the two apart on a text that fits.
 */
export function overflowsClamp(scrollHeight: number, clientHeight: number): boolean {
  return scrollHeight - clientHeight > 1;
}
