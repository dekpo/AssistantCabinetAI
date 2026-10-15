import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { foundPassages, type FoundDocument, type FoundPassage } from "../lib/foundPassages";
import { revealWorkFile, type Evidence } from "../lib/ipc";
import { SummaryPanel } from "./SummaryPanel";

/** « Passages trouvés »: what the search found, above the answer, built from the retrieval alone. The words
 * of the question are marked; a passage found by meaning alone says so and marks nothing. */
export function FoundPassagesBlock({ sources }: { sources: Evidence[] }) {
  const { t } = useTranslation();
  // The document whose summaries are open: they are started by hand, from here.
  const [summarising, setSummarising] = useState<string | null>(null);
  const found = foundPassages(sources);
  if (found.passageCount === 0) {
    return null;
  }

  const pagesOf = (passage: FoundPassage) =>
    passage.lastPage > passage.firstPage
      ? t("found.pages", { from: String(passage.firstPage), to: String(passage.lastPage) })
      : t("found.page", { page: String(passage.firstPage) });

  const passageLine = (passage: FoundPassage) => (
    <li key={passage.chunkId} className="found__passage">
      <span className="found__where">
        {passage.section === null ? pagesOf(passage) : `${passage.section}, ${pagesOf(passage)}`}
        {passage.byMeaning ? <span className="found__meaning"> · {t("found.byMeaning")}</span> : null}
      </span>
      <q className="found__snippet">
        {passage.snippet.map((part, index) =>
          part.marked ? <mark key={index}>{part.text}</mark> : <span key={index}>{part.text}</span>,
        )}
      </q>
    </li>
  );

  const documentBlock = (file: FoundDocument) => (
    <li key={file.relativePath} className="found__document">
      <div className="found__file">
        <span className="found__path">{file.relativePath}</span>
        <span className="found__buttons">
          <button
            type="button"
            className="button button--compact"
            onClick={() => setSummarising(summarising === file.relativePath ? null : file.relativePath)}
          >
            {t("summary.button")}
          </button>
          <button
            type="button"
            className="button button--compact"
            onClick={() => void revealWorkFile(file.relativePath)}
          >
            {t("found.show")}
          </button>
        </span>
      </div>
      {summarising === file.relativePath ? <SummaryPanel relativePath={file.relativePath} /> : null}
      <ul className="found__passages">{file.passages.map(passageLine)}</ul>
    </li>
  );

  return (
    <details className="found" open>
      <summary className="disclosure found__title">
        {t(found.documentCount === 1 ? "found.titleOne" : "found.title", {
          passages: String(found.passageCount),
          documents: String(found.documentCount),
        })}
      </summary>
      <ul className="found__documents">{found.matching.map(documentBlock)}</ul>
      {found.byMeaning.length > 0 ? (
        <details className="found__others">
          <summary className="disclosure">
            {t("found.others", { documents: String(found.byMeaning.length) })}
          </summary>
          <ul className="found__documents">{found.byMeaning.map(documentBlock)}</ul>
        </details>
      ) : null}
    </details>
  );
}
