import type { Translator } from "../i18n/translate";
import { errorMessage, normaliseError } from "./errors";
import type { IndexSummary } from "./ipc";
import { knowledgeLines } from "./knowledgeSummary";
import { counted } from "./plural";
import { unreadFormatsLines } from "./unreadFormats";

/*
 * What the last analysis pass did, as the lines drawn under a folder: one paragraph of short lines
 * for the documents, one for the workbooks. Moved out of the two folder cards so that the explorer
 * and the Settings panel's cards say exactly the same words from one place.
 */

/** The machine codes a pass can report, and the sentence each one deserves. A code we do not
    recognise is dropped rather than shown raw: Rust names capabilities, this file names them in
    her language (`docs/LANGUAGE-AND-LOCALE.md`). */
const CAPABILITY_KEYS: Record<string, string> = {
  ocrEngine: "workFolder.ocrEngineUnavailable",
  pageRasterizer: "workFolder.pageRasterizerUnavailable",
};

/** Renames listed one by one under the analysis summary; the rest are counted. */
const MAX_RENAMES_LISTED = 5;

/** Documents that could not be analysed, listed with their reason; the rest are counted. */
const MAX_FAILED_LISTED = 5;

/** The lines the rename report adds, shared by both passes. */
function renameLines(t: Translator, summary: IndexSummary): string[] {
  if (summary.renamedFiles.length === 0) {
    return [];
  }
  return [
    `${counted(
      summary.renamedFiles.length,
      t("workFolder.renamedOne"),
      t("workFolder.renamedMany"),
    )}.`,
    ...summary.renamedFiles
      .slice(0, MAX_RENAMES_LISTED)
      .map(({ from, to }) => t("workFolder.renamedItem", { from, to })),
    ...(summary.renamedFiles.length > MAX_RENAMES_LISTED
      ? [
          t("workFolder.renamedMore", {
            count: summary.renamedFiles.length - MAX_RENAMES_LISTED,
          }),
        ]
      : []),
  ];
}

/** What the last pass over the documents did, beyond the counts the header already gives. */
export function documentPassLines(t: Translator, summary: IndexSummary | null): string[] {
  if (summary === null) {
    return [];
  }
  return [
    /* First, because it is the one line that asks her to do something: a document that is
       not analysed cannot answer a question, and the reason differs from file to file. */
    ...(summary.failedFiles.length > 0
      ? [
          `${counted(
            summary.failedFiles.length,
            t("workFolder.failedOne"),
            t("workFolder.failedMany"),
          )}.`,
          ...summary.failedFiles.slice(0, MAX_FAILED_LISTED).map(({ path, code, data }) =>
            t("workFolder.failedItem", {
              path,
              reason: errorMessage(t, normaliseError({ code, data })),
            }),
          ),
          ...(summary.failedFiles.length > MAX_FAILED_LISTED
            ? [
                t("workFolder.failedMore", {
                  count: summary.failedFiles.length - MAX_FAILED_LISTED,
                }),
              ]
            : []),
        ]
      : []),
    ...(summary.ocrFiles.length > 0
      ? [`${counted(summary.ocrFiles.length, t("workFolder.ocrOne"), t("workFolder.ocrMany"))}.`]
      : []),
    ...(summary.removedFiles.length > 0
      ? [
          `${counted(
            summary.removedFiles.length,
            t("workFolder.removedOne"),
            t("workFolder.removedMany"),
          )}.`,
        ]
      : []),
    /* Her files were renamed, so say which, old name and new. A count alone would hide the
       one thing she may need to look for. */
    ...renameLines(t, summary),
    ...(summary.renameFailedFiles.length > 0
      ? [
          `${counted(
            summary.renameFailedFiles.length,
            t("workFolder.renameFailedOne"),
            t("workFolder.renameFailedMany"),
          )}.`,
        ]
      : []),
    ...(summary.lowConfidenceFiles.length > 0
      ? [
          `${counted(
            summary.lowConfidenceFiles.length,
            t("workFolder.lowConfidenceOne"),
            t("workFolder.lowConfidenceMany"),
          )}.`,
        ]
      : []),
    /* Files in a format nothing in the product reads: said, so that an answer drawn from the
       others is never mistaken for an answer drawn from the whole folder. */
    ...unreadFormatsLines(t, summary.unreadFormats),
    /* A pass the user stopped, or one the server did not finish: what was read is kept, the
       word search already works on all of it, and the next pass goes on from there. */
    ...(summary.cancelled === true
      ? [t("workFolder.summaryStopped")]
      : (summary.vectorsPending ?? 0) > 0
        ? [
            t("workFolder.summaryMeaningPartial", {
              percent: Math.floor(
                (100 * Math.max(0, summary.chunkCount - (summary.vectorsPending ?? 0))) /
                  Math.max(1, summary.chunkCount),
              ),
            }),
          ]
        : []),
    /* What the knowledge base did with the names, counts only. The line that asks her to act
       (a document whose names were not recorded) comes first among them. */
    ...knowledgeLines(t, summary.knowledge),
    /* Last, and only when something is actually missing: a scan reported unreadable
       because an engine did not start is not the document's fault, and saying so is the
       difference between a five-second diagnosis and an investigation. */
    ...summary.unavailableCapabilities
      .map((code) => CAPABILITY_KEYS[code])
      .filter((key): key is string => key !== undefined)
      .map((key) => t(key)),
  ];
}

/** What the last pass over the workbooks did. It opens with how many were read and where, because
 * the pass is over in a moment: without that line, an Analyse that finishes before the spinner is
 * seen reads as a button that did nothing. */
export function dataPassLines(t: Translator, summary: IndexSummary | null): string[] {
  if (summary === null) {
    return [];
  }
  return [
    t("dataFolder.passDone", {
      count: counted(
        summary.scannedFiles,
        t("dataFolder.spreadsheetOne"),
        t("dataFolder.spreadsheetMany"),
      ),
    }),
    ...(summary.removedFiles.length > 0
      ? [
          `${counted(
            summary.removedFiles.length,
            t("dataFolder.removedOne"),
            t("dataFolder.removedMany"),
          )}.`,
        ]
      : []),
    ...renameLines(t, summary),
    ...(summary.renameFailedFiles.length > 0
      ? [
          `${counted(
            summary.renameFailedFiles.length,
            t("workFolder.renameFailedOne"),
            t("workFolder.renameFailedMany"),
          )}.`,
        ]
      : []),
    /* What the knowledge base read in the tables, counts only. */
    ...knowledgeLines(t, summary.knowledge, "data"),
  ];
}
