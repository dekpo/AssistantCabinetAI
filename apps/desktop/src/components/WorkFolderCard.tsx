import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import { chooseWorkFolder } from "../lib/ipc";
import { ErrorBanner } from "./ErrorBanner";

/** Closing the dialog without choosing is not a failure, so it is not reported as one. */
const CANCELLED = "work_folder_selection_cancelled";

export function WorkFolderCard({
  workFolder,
  onChosen,
}: {
  workFolder: string | null;
  onChosen: (path: string) => void;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);

  const choose = async () => {
    setError(null);
    try {
      onChosen(await chooseWorkFolder());
    } catch (raw: unknown) {
      const failure = normaliseError(raw);
      if (failure.code !== CANCELLED) {
        setError(failure);
      }
    }
  };

  return (
    <section className="card">
      <h2 className="card__title">{t("workFolder.title")}</h2>
      <p className="card__description">{t("workFolder.description")}</p>
      <p className={workFolder === null ? "path path--empty" : "path"}>
        {workFolder ?? t("workFolder.none")}
      </p>
      <button type="button" className="button" onClick={choose}>
        {workFolder === null ? t("workFolder.choose") : t("workFolder.change")}
      </button>
      {error === null ? null : <ErrorBanner error={error} />}
    </section>
  );
}
