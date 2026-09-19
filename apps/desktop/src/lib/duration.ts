/** `2m50s` below one hour, `5s` under a minute. A unit suffix, not a sentence, so it travels as
 * data into the catalogue like a number would (`docs/LANGUAGE-AND-LOCALE.md`). */
export function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.round(milliseconds / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m${seconds}s` : `${seconds}s`;
}
