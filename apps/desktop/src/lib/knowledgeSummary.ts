import type { Translator } from "../i18n/translate";
import type { KnowledgeSummary } from "./ipc";
import { counted } from "./plural";

/**
 * The lines the analysis summary adds when the knowledge base took part. Rust sends counts and
 * machine codes only (`KnowledgeSummary`), so every word here is the catalogue's, in her language
 * (`docs/LANGUAGE-AND-LOCALE.md`), and no name ever reaches this function.
 *
 * Order: the line that asks her to do something first (a document whose names could not be
 * recorded), then what was found, then what was merely read again.
 */
export function knowledgeLines(
  t: Translator,
  summary: KnowledgeSummary | null | undefined,
): string[] {
  if (summary === null || summary === undefined) {
    return [];
  }
  const lines: string[] = [];

  if (summary.errors > 0) {
    lines.push(
      `${counted(summary.errors, t("analysis.knowledge.errorsOne"), t("analysis.knowledge.errorsMany"))}.`,
    );
  }
  if (summary.entitiesDetected > 0) {
    lines.push(
      t("analysis.knowledge.namesSummary", {
        detected: counted(
          summary.entitiesDetected,
          t("analysis.knowledge.detectedOne"),
          t("analysis.knowledge.detectedMany"),
        ),
        known: counted(
          summary.matchedExisting,
          t("analysis.knowledge.knownOne"),
          t("analysis.knowledge.knownMany"),
        ),
      }),
    );
  }
  if (summary.identifiers > 0) {
    lines.push(
      `${counted(
        summary.identifiers,
        t("analysis.knowledge.identifiersOne"),
        t("analysis.knowledge.identifiersMany"),
      )}.`,
    );
  }
  if (summary.refreshedSources > 0) {
    lines.push(
      `${counted(
        summary.refreshedSources,
        t("analysis.knowledge.refreshedOne"),
        t("analysis.knowledge.refreshedMany"),
      )}.`,
    );
  }
  if (summary.truncatedSources > 0) {
    lines.push(
      `${counted(
        summary.truncatedSources,
        t("analysis.knowledge.truncatedOne"),
        t("analysis.knowledge.truncatedMany"),
      )}.`,
    );
  }
  return lines;
}
