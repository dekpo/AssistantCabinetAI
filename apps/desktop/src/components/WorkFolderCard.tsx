import { Fragment, useCallback, useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { analysisFraction, analysisPending } from "../lib/analysis";
import { normaliseError, type AppError } from "../lib/errors";
import {
  chooseWorkFolder,
  ensureSuggestedWorkFolder,
  workFolderInventory,
  type FileRecord,
  type InventoryReport,
} from "../lib/ipc";
import { counted } from "../lib/plural";
import type { IndexingState } from "../state/useIndexing";
import { DocumentGlyph } from "./DocumentGlyph";
import { ErrorBanner } from "./ErrorBanner";

/** Closing the dialog without choosing is not a failure, so it is not reported as one. */
const CANCELLED = "work_folder_selection_cancelled";

/** The machine codes a pass can report, and the sentence each one deserves. A code we do not
    recognise is dropped rather than shown raw: Rust names capabilities, this file names them in
    her language (`docs/LANGUAGE-AND-LOCALE.md`). */
const CAPABILITY_KEYS: Record<string, string> = {
  ocrEngine: "workFolder.ocrEngineUnavailable",
  pageRasterizer: "workFolder.pageRasterizerUnavailable",
};

export function WorkFolderCard({
  workFolder,
  suggestedWorkFolder,
  onChosen,
  indexing,
  detail = "expanded",
}: {
  workFolder: string | null;
  suggestedWorkFolder: string | null;
  onChosen: (path: string) => void;
  /** The one analysis pass, shared with the conversation: an answer that had to ask for it starts
   * the same pass this card's button does. */
  indexing: IndexingState;
  /** How much room the file listing may take. The card lives in two places: beside the
   * conversation, where vertical space is what the answers need, and in the settings panel,
   * where there is room to read the whole folder at once. */
  detail?: "collapsible" | "expanded";
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  /* What is actually in the folder, read from the disk and from the local index. The counts below
     come from here rather than from the last indexing pass, so they stay true after a file is
     added or removed without re-analysing (`docs/WORK-FOLDER-INVENTORY.md`). */
  const [inventory, setInventory] = useState<InventoryReport | null>(null);
  const showSuggestion = workFolder === null && suggestedWorkFolder !== null;
  const { finishedPasses, reset: resetIndexing } = indexing;
  /* Filled while one more pass would still change what the software knows, plain once it would
     not: the button says "do this next" exactly while that is true, and no longer. */
  const pending = inventory !== null && analysisPending(inventory.files);

  const refreshInventory = useCallback(() => {
    if (workFolder === null) {
      setInventory(null);
      return;
    }
    // A failed read leaves the panel without counts rather than with wrong ones.
    workFolderInventory().then(setInventory, () => setInventory(null));
  }, [workFolder]);

  useEffect(refreshInventory, [refreshInventory]);
  // A finished pass changes every file's state, including a pass started from the conversation
  // rather than from this card. Keyed on the count rather than on the summary so a pass that
  // failed halfway still re-reads what it did manage to do.
  useEffect(() => {
    if (finishedPasses > 0) {
      refreshInventory();
    }
  }, [finishedPasses, refreshInventory]);

  /* What the last pass did. One paragraph of short lines rather than three paragraphs: it is one
     fact about one pass, and a blank line between the parts made it read as three separate
     announcements. It lives inside the disclosure, above the listing it describes. */
  const passLines =
    indexing.summary === null
      ? []
      : [
          `${[
            counted(
              indexing.summary.indexedFiles,
              t("workFolder.indexedOne"),
              t("workFolder.indexedMany"),
            ),
            counted(
              indexing.summary.unchangedFiles,
              t("workFolder.unchangedOne"),
              t("workFolder.unchangedMany"),
            ),
            ...(indexing.summary.emptyFiles.length > 0
              ? [
                  counted(
                    indexing.summary.emptyFiles.length,
                    t("workFolder.unreadableOne"),
                    t("workFolder.unreadableMany"),
                  ),
                ]
              : []),
          ].join(", ")}.`,
          ...(indexing.summary.ocrFiles.length > 0
            ? [
                `${counted(
                  indexing.summary.ocrFiles.length,
                  t("workFolder.ocrOne"),
                  t("workFolder.ocrMany"),
                )}.`,
              ]
            : []),
          ...(indexing.summary.lowConfidenceFiles.length > 0
            ? [
                `${counted(
                  indexing.summary.lowConfidenceFiles.length,
                  t("workFolder.lowConfidenceOne"),
                  t("workFolder.lowConfidenceMany"),
                )}.`,
              ]
            : []),
          /* Last, and only when something is actually missing: a scan reported unreadable
             because an engine did not start is not the document's fault, and saying so is the
             difference between a five-second diagnosis and an investigation. */
          ...indexing.summary.unavailableCapabilities
            .map((code) => CAPABILITY_KEYS[code])
            .filter((key): key is string => key !== undefined)
            .map((key) => t(key)),
        ];

  const passSummary =
    passLines.length === 0 ? null : (
      <p className="inventory-detail__summary">
        {passLines.map((line, index) => (
          <Fragment key={index}>
            {index > 0 ? <br /> : null}
            {line}
          </Fragment>
        ))}
      </p>
    );

  const run = async (action: typeof chooseWorkFolder) => {
    setError(null);
    resetIndexing();
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
        <DocumentGlyph />
        {t("workFolder.title")}
      </h2>
      {workFolder !== null ? (
        /* What this folder is for, on hover rather than on screen: the sentence matters while she
           is choosing a folder, and the rest of the time it is four lines the conversation could
           have had. */
        <p className="path" title={t("workFolder.descriptionFull")}>
          {workFolder}
        </p>
      ) : showSuggestion ? (
        <>
          <p className="card__description">{t("workFolder.suggestionLabel")}</p>
          <p className="path" title={t("workFolder.descriptionFull")}>
            {suggestedWorkFolder}
          </p>
          <p className="card__description">{t("workFolder.suggestionWhy")}</p>
        </>
      ) : (
        <p className="path path--empty" title={t("workFolder.descriptionFull")}>
          {t("workFolder.none")}
        </p>
      )}
      <div className="work-folder__actions">
        {showSuggestion ? (
          <button
            type="button"
            className="button button--compact button--primary"
            disabled={busy}
            onClick={() => void run(ensureSuggestedWorkFolder)}
          >
            {t("workFolder.createSuggested")}
          </button>
        ) : null}
        <div className="work-folder__row">
          <button
            type="button"
            className="button button--compact"
            disabled={busy}
            onClick={() => void run(chooseWorkFolder)}
          >
            {workFolder === null ? t("workFolder.choose") : t("workFolder.change")}
          </button>
          {workFolder !== null ? (
            <AnalyseButton indexing={indexing} emphasised={pending} />
          ) : null}
        </div>
        {/* Under both buttons, for the length of the pass and no longer. A spinner says only
            "still working"; on a folder of scans, which is minutes rather than seconds, what she
            needs is whether it is nearly done. */}
        {indexing.progress === null ? null : (
          <AnalysisProgress
            fraction={analysisFraction(indexing.progress)}
            label={t("workFolder.progressLabel")}
          />
        )}
      </div>
      {inventory === null ? (
        /* No inventory to put it under - a folder that could not be read still owes her an
           account of the pass that just ran. */
        passSummary
      ) : (
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
          {inventory.summary.totalFiles === 0 ? (
            passSummary
          ) : detail === "collapsible" ? (
            /* The same disclosure as the sources under an answer, for the same reason: it says
               what is behind it in one line and gives the room back when it is closed. What the
               last pass did goes inside it, above the listing, because it is a note about that
               listing rather than a fourth fact about the card. */
            <details className="inventory-detail">
              <summary className="disclosure">{t("workFolder.detailToggle")}</summary>
              {passSummary}
              <FileList files={inventory.files} />
            </details>
          ) : (
            <>
              {passSummary}
              <FileList files={inventory.files} />
            </>
          )}
        </>
      )}
      {error === null ? null : <ErrorBanner error={error} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
    </section>
  );
}

/**
 * The one control that starts a pass. Filled with the accent colour only while a pass would still
 * change something - once every file has been read it is an ordinary button beside "change the
 * folder", because by then it is a thing she may do, not the thing she must do first.
 */
function AnalyseButton({
  indexing,
  emphasised,
}: {
  indexing: IndexingState;
  emphasised: boolean;
}) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      className={
        emphasised
          ? "button button--compact button--primary work-folder__analyse"
          : "button button--compact work-folder__analyse"
      }
      disabled={indexing.running}
      onClick={() => void indexing.run()}
    >
      {indexing.running ? (
        <span className="message__pending">
          <span className="spinner" aria-hidden="true" />
          {t("actions.analyzing")}
        </span>
      ) : (
        t("actions.analyze")
      )}
    </button>
  );
}

/** A hairline that fills as the pass walks the folder. Gone the moment the pass ends. */
function AnalysisProgress({ fraction, label }: { fraction: number; label: string }) {
  return (
    <div
      className="progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(fraction * 100)}
    >
      <div className="progress__fill" style={{ width: `${fraction * 100}%` }} />
    </div>
  );
}

/** One file per row: what it is called on its own line, and what happened to it underneath. The
 * name is the thing she reads, so it gets the width; the state is a note about it, not a column
 * competing with it. The same two facts a deterministic answer prints, so the panel and an answer
 * can never disagree. */
function FileList({ files }: { files: FileRecord[] }) {
  const { t } = useTranslation();

  return (
    <ul className="inventory">
      {files.map((file) => (
        <li key={file.id + file.relativePath} className="inventory__file">
          {/* One line, whatever the path costs: a name broken across two lines in a narrow
              sidebar is harder to scan than one that ends in an ellipsis, and the full path is
              on hover for the rare name long enough to need it. */}
          <span className="inventory__path" title={file.relativePath}>
            {file.relativePath}
          </span>
          <span className="inventory__state">
            {t(`folderAnswer.processing.${file.processingStatus}`)}
          </span>
        </li>
      ))}
    </ul>
  );
}
