/**
 * The limits of the targeting settings, as the settings dialog offers them. Rust clamps whatever
 * it is sent (`Settings::targeting_policy`), so these are the same bounds repeated for the control:
 * a number input has to know them before anything is saved. A test reads the Rust source and fails
 * if the two drift apart.
 */

export interface Limit {
  min: number;
  max: number;
}

/** `knowledgeMinScopeFiles`: files on one side below which nothing is narrowed by itself. */
export const MIN_SCOPE_FILES: Limit = { min: 1, max: 10_000 };

/** `knowledgeMaxSelectivity`, shown as a percentage of the selection. */
export const MAX_SELECTIVITY: Limit = { min: 0.05, max: 1 };

/** `knowledgeMinReduction`, shown as a percentage of the selection. */
export const MIN_REDUCTION: Limit = { min: 0, max: 0.95 };

/** `knowledgeMaxCandidates`: documents searched after a narrowing. */
export const MAX_CANDIDATES: Limit = { min: 1, max: 500 };

/** `knowledgeWidenBelowScore`: a similarity, 0 to 1.5 (the scale belongs to the embedding model). */
export const WIDEN_BELOW_SCORE: Limit = { min: 0, max: 1.5 };

/** A typed number inside its limit. Anything that is not a number keeps `fallback`, the value that
 * was stored: an empty field must not turn a setting into zero. */
export function boundedNumber(
  draft: string,
  limit: Limit,
  fallback: number,
  digits = 0,
): number {
  const typed = Number(draft.trim().replace(",", "."));
  if (draft.trim() === "" || !Number.isFinite(typed)) {
    return fallback;
  }
  const clamped = Math.min(Math.max(typed, limit.min), limit.max);
  const factor = 10 ** digits;
  return Math.round(clamped * factor) / factor;
}

/** A share as a whole percentage for the field (`0.6` is `"60"`). */
export function percentDraft(fraction: number): string {
  return String(Math.round(fraction * 100));
}

/** A percentage typed in the field as the share Rust stores (`"60"` is `0.6`), inside its limit. */
export function fractionFromPercentDraft(
  draft: string,
  limit: Limit,
  fallback: number,
): number {
  const percent = boundedNumber(
    draft,
    { min: limit.min * 100, max: limit.max * 100 },
    fallback * 100,
    0,
  );
  return percent / 100;
}
