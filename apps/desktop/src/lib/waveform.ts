/** How much each bar of the waveform follows the input level: the middle bars move most. */
const PROFILE = [0.45, 0.7, 0.9, 1, 0.9, 0.7, 0.45] as const;

/** The bar never disappears, so the pill reads as "listening" even in silence (percent of its height). */
const MIN_PERCENT = 12;

/**
 * Heights, in percent, of the bars of the recording pill for an input level between 0 (silence) and
 * 1 (loud). Pure, so the part of the animation that is arithmetic is tested; a level that is not a
 * finite number reads as silence.
 */
export function barHeights(level: number): number[] {
  const clamped = Number.isFinite(level) ? Math.min(1, Math.max(0, level)) : 0;
  return PROFILE.map((weight) =>
    Math.round(MIN_PERCENT + (100 - MIN_PERCENT) * clamped * weight),
  );
}

/** `m:ss` of a running recording; a unit-free clock, so it needs no catalogue sentence. */
export function formatClock(milliseconds: number): string {
  const total = Math.max(0, Math.floor(milliseconds / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}
