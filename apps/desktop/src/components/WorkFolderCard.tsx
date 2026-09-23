import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { normaliseError, type AppError } from "../lib/errors";
import {
  chooseWorkFolder,
  ensureSuggestedWorkFolder,
  indexWorkFolder,
  workFolderInventory,
  type FileRecord,
  type IndexSummary,
  type InventoryReport,
} from "../lib/ipc";
import { counted } from "../lib/plural";
import { ErrorBanner } from "./ErrorBanner";

/** Closing the dialog without choosing is not a failure, so it is not reported as one. */
const CANCELLED = "work_folder_selection_cancelled";

export function WorkFolderCard({
  workFolder,
  suggestedWorkFolder,
  onChosen,
  detail = "expanded",
}: {
  workFolder: string | null;
  suggestedWorkFolder: string | null;
  onChosen: (path: string) => void;
  /** How much room the file listing may take. The card lives in two places: beside the
   * conversation, where vertical space is what the answers need, and in the settings panel,
   * where there is room to read the whole folder at once. */
  detail?: "collapsible" | "expanded";
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [indexing, setIndexing] = useState(false);
  const [indexSummary, setIndexSummary] = useState<IndexSummary | null>(null);
  /* What is actually in the folder, read from the disk and from the local index. The counts below
     come from here rather than from the last indexing pass, so they stay true after a file is
     added or removed without re-analysing (`docs/WORK-FOLDER-INVENTORY.md`). */
  const [inventory, setInventory] = useState<InventoryReport | null>(null);
  const showSuggestion = workFolder === null && suggestedWorkFolder !== null;

  const refreshInventory = useCallback(() => {
    if (workFolder === null) {
      setInventory(null);
      return;
    }
    // A failed read leaves the panel without counts rather than with wrong ones.
    workFolderInventory().then(setInventory, () => setInventory(null));
  }, [workFolder]);

  useEffect(refreshInventory, [refreshInventory]);

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
      refreshInventory();
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
      {inventory === null ? null : (
        <>
          <p className="card__description">
            {`${[
              counted(
                inventory.summary.totalFiles,
                t("workFolder.filesOne"),
                t("workFolder.filesMany"),
              ),
              counted(
                inventory.summary.indexedFiles,
                t("workFolder.indexedOne"),
                t("workFolder.indexedMany"),
              ),
              counted(
                inventory.summary.unreadableFiles,
                t("workFolder.unreadableOne"),
                t("workFolder.unreadableMany"),
              ),
            ].join(", ")}.`}
          </p>
          {inventory.summary.totalFiles === 0 ? null : detail === "collapsible" ? (
            /* The same disclosure as the sources under an answer, for the same reason: it says
               what is behind it in one line and gives the room back when it is closed. */
            <details className="inventory-detail">
              <summary className="disclosure">{t("workFolder.detailToggle")}</summary>
              <FileList files={inventory.files} />
            </details>
          ) : (
            <FileList files={inventory.files} />
          )}
        </>
      )}
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

/** One line per file: what it is called, and what happened to it. The same two facts a
 * deterministic answer prints, so the panel and an answer can never disagree. */
function FileList({ files }: { files: FileRecord[] }) {
  const { t } = useTranslation();

  return (
    <ul className="inventory">
      {files.map((file) => (
        <li key={file.id + file.relativePath} className="inventory__file">
          <span className="inventory__path">{file.relativePath}</span>
          <span className="inventory__state">
            {t(`folderAnswer.processing.${file.processingStatus}`)}
          </span>
        </li>
      ))}
    </ul>
  );
}
