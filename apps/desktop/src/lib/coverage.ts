import type { Translator } from "../i18n/translate";
import type { EvidenceCoverage } from "./ipc";
import { counted } from "./plural";

/**
 * The line under an answer that asked about every document.
 *
 * It exists because an answer that covered six of ten analysed files reads exactly like an answer
 * that covered all ten. Rust computes the two numbers from the evidence it assembled and from the
 * inventory, so this sentence cannot be talked out of by the model - it is written here, beside
 * the answer, not inside it (`docs/WORK-FOLDER-INVENTORY.md`).
 *
 * Files nothing could be read from are named in the same breath, because "10 of 10 documents" is
 * only the whole truth when the folder holds nothing else.
 *
 * A complete answer with nothing unreadable has nothing to disclose, so it gets no line: the line
 * is there to reveal a gap, and "1 of 1" only repeats what she chose.
 */
export function coverageLine(t: Translator, coverage: EvidenceCoverage): string | null {
  if (coverage.filesCovered >= coverage.indexedFiles && coverage.unreadableFiles === 0) {
    return null;
  }
  const documents = counted(
    coverage.indexedFiles,
    t("chat.coverageDocumentOne"),
    t("chat.coverageDocumentMany"),
  );
  const parts = [t("chat.coverage", { covered: coverage.filesCovered, documents })];
  if (coverage.unreadableFiles > 0) {
    parts.push(
      t("chat.coverageUnreadable", {
        files: counted(
          coverage.unreadableFiles,
          t("chat.coverageFileOne"),
          t("chat.coverageFileMany"),
        ),
      }),
    );
  }
  return parts.join(" ");
}
