import { Fragment, useCallback, useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { analysisFraction, analysisPending, countPending } from "../lib/analysis";
import { normaliseError, type AppError } from "../lib/errors";
import {
  chooseWorkFolder,
  ensureSuggestedWorkFolder,
  resetIndex,
  revealWorkFolder,
  workFolderInventory,
  type FileRecord,
  type InventoryReport,
} from "../lib/ipc";
import { counted } from "../lib/plural";
import type { IndexingState } from "../state/useIndexing";
import { ConfirmDialog } from "./ConfirmDialog";
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
  const [confirmingReset, setConfirmingReset] = useState(false);
  const [resetting, setResetting] = useState(false);
  const showSuggestion = workFolder === null && suggestedWorkFolder !== null;
  const { finishedPasses, reset: resetIndexing } = indexing;
  /* Filled while one more pass would still change what the software knows, plain once it would
     not: the button says "do this next" exactly while that is true, and no longer. */
  const pending = inventory !== null && analysisPending(inventory.files);
  /* How many documents that pass would change. The button already said "there is something to
     do"; the number says how much, which is the difference between a badge she learns to ignore
     and one she acts on. */
  const pendingCount = inventory === null ? 0 : countPending(inventory.files);

  const refreshInventory = useCallback(() => {
    if (workFolder === null) {
      setInventory(null);
      return;
    }
    // A failed read leaves the panel without counts rather than with wrong ones.
    workFolderInventory().then(setInventory, () => setInventory(null));
  }, [workFolder]);

  useEffect(refreshInventory, [refreshInventory]);

  /* She adds a document the way anyone does: in Explorer or Finder, with this window behind the
     other one. Coming back to the window is therefore the moment the panel is most likely to be
     out of date, and the only moment we can catch without watching the filesystem - which would
     mean a background watcher, debouncing a scanner writing a file in pieces, and a cloud-sync
     folder churning underneath it, for no more than the same answer sooner.
     `focus` rather than `visibilitychange`: alt-tabbing back to a window that was never hidden
     fires only the first. Rust re-reads a file's bytes only when its size or time moved
     (`inventory::FileHashCache`), so doing this often is cheap. */
  useEffect(() => {
    if (workFolder === null) {
      return;
    }
    window.addEventListener("focus", refreshInventory);
    return () => window.removeEventListener("focus", refreshInventory);
  }, [workFolder, refreshInventory]);
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
          ...(indexing.summary.removedFiles.length > 0
            ? [
                `${counted(
                  indexing.summary.removedFiles.length,
                  t("workFolder.removedOne"),
                  t("workFolder.removedMany"),
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

  /* Opening the folder is the one action here that changes nothing, so it reports a failure and
     otherwise leaves the card exactly as it was - no busy state, no cleared error. */
  const reveal = async () => {
    setError(null);
    try {
      await revealWorkFolder();
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    }
  };

  const confirmReset = async () => {
    setError(null);
    setResetting(true);
    try {
      await resetIndex();
      // The pass summary describes an index that no longer exists, so it goes with it. The
      // inventory is then re-read, and every document comes back as never analysed, which lights
      // the Analyse button by the same rule as any other unanalysed folder.
      resetIndexing();
      refreshInventory();
      setConfirmingReset(false);
    } catch (raw: unknown) {
      setError(normaliseError(raw));
      setConfirmingReset(false);
    } finally {
      setResetting(false);
    }
  };

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
          {/* First, because it is the only one that changes nothing: it hands the folder to the
              file manager she already uses. Which one that is, is Rust's business. */}
          {workFolder !== null ? (
            <button
              type="button"
              className="button button--compact"
              title={t("workFolder.revealHint")}
              onClick={() => void reveal()}
            >
              {t("workFolder.reveal")}
            </button>
          ) : null}
          <button
            type="button"
            className="button button--compact"
            /* The row holds four controls in a sidebar column, so the label is one word and the
               sentence it shortens lives on hover. The card's own heading already says which
               folder this is. */
            title={workFolder === null ? undefined : t("workFolder.changeHint")}
            disabled={busy}
            onClick={() => void run(chooseWorkFolder)}
          >
            {workFolder === null ? t("workFolder.choose") : t("workFolder.change")}
          </button>
          {/* Between the two it sits between, because that is what it undoes: it returns the
              folder to the state "chosen, never analysed" that the button on its left leaves it
              in and the button on its right leaves. Plain styling like its neighbours - it is a
              standing action, not the thing to do next; the warning belongs on the
              confirmation. */}
          {workFolder !== null ? (
            <button
              type="button"
              className="button button--compact"
              disabled={busy || indexing.running || resetting}
              onClick={() => setConfirmingReset(true)}
            >
              {t("workFolder.reset")}
            </button>
          ) : null}
          {workFolder !== null ? (
            <AnalyseButton
              indexing={indexing}
              emphasised={pending}
              pendingCount={pendingCount}
            />
          ) : null}
        </div>
        {/* Under the whole row, for the length of the pass and no longer. A spinner says only
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
              {/* The panel this card sits in is wide, so the listing gets its second column. */}
              <FileList files={inventory.files} wide />
            </>
          )}
        </>
      )}
      {error === null ? null : <ErrorBanner error={error} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
      {confirmingReset ? (
        <ConfirmDialog
          title={t("workFolder.resetTitle")}
          body={t("workFolder.resetBody")}
          confirmLabel={t("workFolder.reset")}
          destructive
          busy={resetting}
          onConfirm={() => void confirmReset()}
          onCancel={() => setConfirmingReset(false)}
        />
      ) : null}
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
  pendingCount,
}: {
  indexing: IndexingState;
  emphasised: boolean;
  /** How many documents the pass would still change. Shown only while it is worth acting on. */
  pendingCount: number;
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
      ) : pendingCount > 0 ? (
        /* "Analyser (3)". The count rides inside the label rather than in a separate badge: the
           row already carries three controls, and a badge would be a fourth thing competing for
           the same few pixels. */
        `${t("actions.analyze")} (${pendingCount})`
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

/** One file per row, and the same two facts a deterministic answer prints, so the panel and an
 * answer can never disagree.
 *
 * Two shapes for one markup, chosen by where the card is rather than by a breakpoint - the card's
 * width is decided by which of its two homes it is in, not by the size of the window. In the
 * settings panel there is room to put the state beside the name and align the states down the
 * right, which is how a listing is normally read. In the sidebar the same two columns would leave
 * the name a few characters wide, so the state goes underneath it and the name keeps the width. */
function FileList({ files, wide = false }: { files: FileRecord[]; wide?: boolean }) {
  const { t } = useTranslation();

  return (
    <ul className={wide ? "inventory inventory--wide" : "inventory"}>
      {files.map((file) => (
        <li key={file.id + file.relativePath} className="inventory__file">
          {/* One line, whatever the path costs: a name broken across two lines is harder to scan
              than one that ends in an ellipsis, and the full path is on hover for the rare name
              long enough to need it. */}
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
