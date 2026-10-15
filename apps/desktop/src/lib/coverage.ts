import type { Translator } from "../i18n/translate";
import type { EvidenceCoverage } from "./ipc";
import { counted } from "./plural";

/**
 * The line under an answer that rests on several documents.
 *
 * It exists because an answer that covered six of ten analysed files reads exactly like an answer
 * that covered all ten, and an answer drawn from three documents of a library of 885 reads like one
 * drawn from the whole library. Rust computes the numbers from the evidence it assembled and from the
 * index, so this sentence cannot be talked out of by the model - it is written here, beside the
 * answer, not inside it (`docs/WORK-FOLDER-INVENTORY.md`).
 *
 * The count is always given, never a bare "complete": "4 of 4" says what was read. A library of one
 * document with nothing to disclose gets no line ("1 of 1" only repeats what the user chose), and a
 * document the user named gets no coverage at all (Rust sends none).
 *
 * Files nothing could be read from are named in the same breath, and so is a search by meaning that
 * did not see every passage: either would make the count read as more than it is.
 */
export function coverageLine(t: Translator, coverage: EvidenceCoverage): string | null {
  const countShown = coverage.indexedFiles > 1 || coverage.unreadableFiles > 0;
  const vectorsMissing = coverage.unvectorisedChunks > 0;
  const otherModel = coverage.mismatchedChunks > 0;
  if (!countShown && !vectorsMissing && !otherModel) {
    return null;
  }
  const parts: string[] = [];
  if (countShown) {
    const documents = counted(
      coverage.indexedFiles,
      t("chat.coverageDocumentOne"),
      t("chat.coverageDocumentMany"),
    );
    parts.push(t("chat.coverage", { covered: coverage.filesCovered, documents }));
  }
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
  const passages = (count: number) =>
    counted(count, t("chat.coveragePassageOne"), t("chat.coveragePassageMany"));
  if (vectorsMissing) {
    parts.push(
      t("chat.coverageVectorsMissing", { passages: passages(coverage.unvectorisedChunks) }),
    );
  }
  if (otherModel) {
    parts.push(
      t("chat.coverageVectorsOtherModel", { passages: passages(coverage.mismatchedChunks) }),
    );
  }
  return parts.join(" ");
}
