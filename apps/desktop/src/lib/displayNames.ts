import type { Translator } from "../i18n/translate";

/**
 * Internal names a model may repeat to the user, turned into the names the application itself uses
 * (HAP-1, BUG-10; owner decision of 4 October 2026, `docs/DECISIONS.md`).
 *
 * A block or label the model reads - `WORK_FOLDER_CONTEXT`, `Table results`, `Document excerpts` -
 * is the only name it was given for what it looked at, so asked "where did you look" it says that
 * name. The user knows "Documents folder" and "Data folder". This is a presentation-layer
 * substitution of identifiers only: the stored message and the history sent back to the model keep
 * the model's own words, and a sentence, a number or a citation is never touched.
 *
 * One entry per internal name; `displayNames.test.ts` fails when a name a model reads has none.
 */
const INTERNAL_NAMES: ReadonlyArray<{ pattern: RegExp; key: string }> = [
  { pattern: /\bWORK[_ ]FOLDER[_ ](?:KNOWLEDGE[_ ]CONTRACT|CONTEXT)\b/gi, key: "names.documentsFolder" },
  { pattern: /\bDocument[_ ]excerpts\b/gi, key: "names.documentsFolder" },
  { pattern: /\bTable[_ ]r(?:esults|ésultats)\b/gi, key: "names.dataFolder" },
];

export function localiseInternalNames(text: string, t: Translator): string {
  let shown = text;
  for (const { pattern, key } of INTERNAL_NAMES) {
    shown = shown.replace(pattern, () => t(key));
  }
  return shown;
}

/** The internal identifiers this module covers, as written in what the model reads. */
export const COVERED_INTERNAL_NAMES: readonly string[] = [
  "WORK_FOLDER_CONTEXT",
  "WORK FOLDER KNOWLEDGE CONTRACT",
  "Document excerpts",
  "Table results",
];
