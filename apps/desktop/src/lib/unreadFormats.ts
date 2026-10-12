import type { Translator } from "../i18n/translate";
import type { UnreadFormat } from "./ipc";
import { counted } from "./plural";

/**
 * The line of the analysis summary that says how many files were not read, and in which formats
 * (`workFolder.unread*`). Rust sends a count per extension and never a file name
 * (`IndexSummary.unreadFormats`); every word is the catalogue's, in her language. A folder whose
 * files are all read has no such line.
 */
export function unreadFormatsLines(t: Translator, formats: UnreadFormat[] | undefined): string[] {
  if (formats === undefined || formats.length === 0) {
    return [];
  }
  const total = formats.reduce((sum, format) => sum + format.count, 0);
  const list = formats.map((format) => t("workFolder.unreadFormat", { ...format })).join(", ");
  return [
    `${counted(total, t("workFolder.unreadOne"), t("workFolder.unreadMany"))} (${list}).`,
  ];
}
