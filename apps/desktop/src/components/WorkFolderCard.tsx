import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import { chooseWorkFolder, ensureSuggestedWorkFolder } from "../lib/ipc";
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
  const showSuggestion = workFolder === null && suggestedWorkFolder !== null;

  const run = async (action: typeof chooseWorkFolder) => {
    setError(null);
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
        <button
          type="button"
          className="button"
          disabled={busy}
          onClick={() => void run(chooseWorkFolder)}
        >
          {workFolder === null ? t("workFolder.choose") : t("workFolder.change")}
        </button>
      </div>
      {error === null ? null : <ErrorBanner error={error} />}
    </section>
  );
}
