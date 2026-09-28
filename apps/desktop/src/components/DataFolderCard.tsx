import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import { abbreviateFolderPath } from "../lib/folderPath";
import { chooseDataFolder, ensureSuggestedDataFolder, revealDataFolder } from "../lib/ipc";
import { ErrorBanner } from "./ErrorBanner";
import { TableGlyph } from "./TableGlyph";

/** Closing the dialog without choosing is not a failure, so it is not reported as one. Rust's
 * `choose_data_folder` returns the same code as the Documents Folder dialog rather than a second
 * one of its own, so this mirrors `WorkFolderCard`'s check exactly. */
const CANCELLED = "work_folder_selection_cancelled";

/**
 * The Data Folder's card: choose, see the suggested path, create it, change it, reveal it in the
 * file manager. Deliberately smaller than `WorkFolderCard` - there is no spreadsheet parsing or
 * indexing yet (`docs/DECISIONS.md`), so there is nothing to analyse, no file listing to show and
 * no index to reset. Same markup classes as the Documents Folder card, so the two look and behave
 * alike wherever a control is actually offered.
 */
export function DataFolderCard({
  dataFolder,
  suggestedDataFolder,
  onChosen,
  compact = false,
}: {
  dataFolder: string | null;
  suggestedDataFolder: string | null;
  onChosen: (path: string) => void;
  /** The sidebar box has room for a folder name, not a whole path; the settings panel has room
   * for the whole thing. The one axis `WorkFolderCard`'s `detail` prop varies that this card
   * actually needs, since it holds no file listing to lay out differently. */
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const showSuggestion = dataFolder === null && suggestedDataFolder !== null;
  const shownPath = (path: string) => (compact ? abbreviateFolderPath(path) : path);

  /* Opening the folder is the one action here that changes nothing, so it reports a failure and
     otherwise leaves the card exactly as it was. */
  const reveal = async () => {
    setError(null);
    try {
      await revealDataFolder();
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    }
  };

  const run = async (action: typeof chooseDataFolder) => {
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
      <h2 className="card__title">
        <TableGlyph />
        {t("dataFolder.title")}
      </h2>
      {dataFolder !== null ? (
        <p className="path" title={t("dataFolder.descriptionFull")}>
          {shownPath(dataFolder)}
        </p>
      ) : showSuggestion ? (
        <>
          <p className="card__description">{t("dataFolder.suggestionLabel")}</p>
          <p className="path" title={t("dataFolder.descriptionFull")}>
            {suggestedDataFolder === null ? null : shownPath(suggestedDataFolder)}
          </p>
          <p className="card__description">{t("dataFolder.suggestionWhy")}</p>
        </>
      ) : (
        <p className="path path--empty" title={t("dataFolder.descriptionFull")}>
          {t("dataFolder.none")}
        </p>
      )}
      <div className="work-folder__actions">
        {showSuggestion ? (
          <button
            type="button"
            className="button button--compact button--primary"
            disabled={busy}
            onClick={() => void run(ensureSuggestedDataFolder)}
          >
            {t("dataFolder.createSuggested")}
          </button>
        ) : null}
        <div className="work-folder__row">
          {/* First, because it is the only one that changes nothing, same ordering as the
              Documents Folder row. */}
          {dataFolder !== null ? (
            <button
              type="button"
              className="button button--compact"
              title={t("dataFolder.revealHint")}
              onClick={() => void reveal()}
            >
              {t("dataFolder.reveal")}
            </button>
          ) : null}
          <button
            type="button"
            className="button button--compact"
            title={dataFolder === null ? undefined : t("dataFolder.changeHint")}
            disabled={busy}
            onClick={() => void run(chooseDataFolder)}
          >
            {dataFolder === null ? t("dataFolder.choose") : t("dataFolder.change")}
          </button>
        </div>
      </div>
      {error === null ? null : <ErrorBanner error={error} />}
    </section>
  );
}
