import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import {
  chooseWorkFolder,
  ensureSuggestedWorkFolder,
  indexWorkFolder,
  type IndexSummary,
} from "../lib/ipc";
import { counted } from "../lib/plural";
import { ErrorBanner } from "./ErrorBanner";

/** Closing the dialog without choosing is not a failure, so it is not reported as one. */
const CANCELLED = "work_folder_selection_cancelled";

export function WorkFolderCard({
  workFolder,
  suggestedWorkFolder,
  onChosen,
}: {
  workFolder: string | null;
  suggestedWorkFolder: string | null;
  onChosen: (path: string) => void;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [indexing, setIndexing] = useState(false);
  const [indexSummary, setIndexSummary] = useState<IndexSummary | null>(null);
  const showSuggestion = workFolder === null && suggestedWorkFolder !== null;

  const run = async (action: typeof chooseWorkFolder) => {
    setError(null);
    setIndexSummary(null);
    setBusy(true);
    try {
      onChosen(await action());
    } catch (raw: unknown) {
      const failure = normaliseError(raw);
      if (failure.code !== CANCELLED) {
        setError(failure);
      }
    } finally {
      setBusy(false);
    }
  };

  const runIndexing = async () => {
    setError(null);
    setIndexing(true);
    try {
      setIndexSummary(await indexWorkFolder());
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    } finally {
      setIndexing(false);
    }
  };

  return (
    <section className="card">
      <h2 className="card__title">{t("workFolder.title")}</h2>
      <p className="card__description">{t("workFolder.description")}</p>
      {workFolder !== null ? (
        <p className="path">{workFolder}</p>
      ) : showSuggestion ? (
        <>
          <p className="card__description">{t("workFolder.suggestionLabel")}</p>
          <p className="path">{suggestedWorkFolder}</p>
          <p className="card__description">{t("workFolder.suggestionWhy")}</p>
        </>
      ) : (
        <p className="path path--empty">{t("workFolder.none")}</p>
      )}
      <div className="work-folder__actions">
        {showSuggestion ? (
          <button
            type="button"
            className="button button--primary"
            disabled={busy}
            onClick={() => void run(ensureSuggestedWorkFolder)}
          >
            {t("workFolder.createSuggested")}
          </button>
        ) : null}
        <div className="work-folder__row">
          <button
            type="button"
            className="button"
            disabled={busy}
            onClick={() => void run(chooseWorkFolder)}
          >
            {workFolder === null ? t("workFolder.choose") : t("workFolder.change")}
          </button>
          {workFolder !== null ? (
            <button
              type="button"
              className="button button--primary"
              disabled={indexing}
              onClick={() => void runIndexing()}
            >
              {indexing ? (
                <span className="message__pending">
                  <span className="spinner" aria-hidden="true" />
                  {t("workFolder.indexing")}
                </span>
              ) : (
                t("workFolder.indexAction")
              )}
            </button>
          ) : null}
        </div>
      </div>
      {indexSummary === null ? null : (
        <>
          <p className="card__description">
            {`${[
              counted(
                indexSummary.indexedFiles,
                t("workFolder.indexedOne"),
                t("workFolder.indexedMany"),
              ),
              counted(
                indexSummary.unchangedFiles,
                t("workFolder.unchangedOne"),
                t("workFolder.unchangedMany"),
              ),
              ...(indexSummary.emptyFiles.length > 0
                ? [
                    counted(
                      indexSummary.emptyFiles.length,
                      t("workFolder.unreadableOne"),
                      t("workFolder.unreadableMany"),
                    ),
                  ]
                : []),
            ].join(", ")}.`}
          </p>
          {indexSummary.ocrFiles.length > 0 ? (
            <p className="card__description">
              {`${counted(
                indexSummary.ocrFiles.length,
                t("workFolder.ocrOne"),
                t("workFolder.ocrMany"),
              )}.`}
            </p>
          ) : null}
          {indexSummary.lowConfidenceFiles.length > 0 ? (
            <p className="card__description">
              {`${counted(
                indexSummary.lowConfidenceFiles.length,
                t("workFolder.lowConfidenceOne"),
                t("workFolder.lowConfidenceMany"),
              )}.`}
            </p>
          ) : null}
        </>
      )}
      {error === null ? null : <ErrorBanner error={error} />}
    </section>
  );
}
