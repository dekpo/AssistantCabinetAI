/** A type rather than an interface, so it passes straight into the translator's values record. */
export type TimestampParts = {
  year: string;
  month: string;
  day: string;
  hour: string;
  minute: string;
  second: string;
};

/**
 * Zero-padded parts of a local wall-clock time, 24-hour. Data, not a sentence: the catalogue
 * orders them, because `2026-09-21` and `21-09-2026` are the same instant in two languages
 * (`docs/LANGUAGE-AND-LOCALE.md`). `Intl.DateTimeFormat` is deliberately not used - it writes
 * `09/21/2026, 4:51:03 PM` for `en-US`, which is not the format this product shows.
 */
export function timestampParts(milliseconds: number): TimestampParts {
  const date = new Date(milliseconds);
  return {
    year: String(date.getFullYear()).padStart(4, "0"),
    month: pad(date.getMonth() + 1),
    day: pad(date.getDate()),
    hour: pad(date.getHours()),
    minute: pad(date.getMinutes()),
    second: pad(date.getSeconds()),
  };
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}
