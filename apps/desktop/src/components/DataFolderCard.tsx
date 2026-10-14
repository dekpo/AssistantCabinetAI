import { Fragment, useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { analysisFraction, analysisPending, countPending } from "../lib/analysis";
import { scopedPaths, scopeFromSearch, selectionOf } from "../lib/analysisScope";
import { shownFiles } from "../lib/finderField";
import { normaliseError, type AppError } from "../lib/errors";
import { abbreviateFolderPath } from "../lib/folderPath";
import { knowledgeLines } from "../lib/knowledgeSummary";
import {
  chooseDataFolder,
  dataFolderInventory,
  ensureSuggestedDataFolder,
  resetDataIndex,
  revealDataFile,
  revealDataFolder,
  type AnalysisScope,
  type DataFolderReport,
  type FileRecord,
} from "../lib/ipc";
import { counted } from "../lib/plural";
import { useFinderSearch } from "../state/useFinderSearch";
import type { IndexingState } from "../state/useIndexing";
import { useProfileStatus } from "../state/useProfileStatus";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import {
  AnalyseButton,
  AnalysisProgress,
  FileList,
  isSelectable,
  KnowledgeLink,
  SelectAll,
  StatusDot,
} from "./FolderCardParts";
import { FinderControls } from "./FinderControls";
import { TableGlyph } from "./TableGlyph";
import { WarningGlyph } from "./WarningGlyph";

/** Closing the dialog without choosing is not a failure, so it is not reported as one. Rust's
 * `choose_data_folder` returns the same code as the Documents Folder dialog. */
const CANCELLED = "work_folder_selection_cancelled";

/** Renames listed one by one under the analysis summary; the rest are counted. */
const MAX_RENAMES_LISTED = 5;

/**
 * The Data Folder's card: the Documents Folder card, cloned (`docs/SESSION-DATA-05-TABULAR-UI.md`
 * section 6). The same markup and classes, the same buttons - Voir, Changer, Reset, Analyser - the
 * same three counts and coloured dots, and the same selection list. Where the two differ, it is in
 * words only: a workbook is green when it holds a usable table, red when it holds none, and its
 * Analyse reads spreadsheets on this computer, without the AI, in a moment.
 *
 * Unlike the Documents Folder, choosing one is never required: the conversation works without it.
 */
export function DataFolderCard({
  dataFolder,
  suggestedDataFolder,
  onChosen,
  indexing,
  detail = "expanded",
  scope,
  onScopeChange,
  scopeLocked = false,
  onOpenKnowledge,
  suggestions,
}: {
  dataFolder: string | null;
  suggestedDataFolder: string | null;
  onChosen: (path: string) => void;
  /** The Data Folder's own analysis pass, shared by the sidebar and the settings panel. */
  indexing: IndexingState;
  /** Beside the conversation the listing folds away and doubles as the selection; in the settings
   * panel it is shown whole, with a "See" button per file. */
  detail?: "collapsible" | "expanded";
  /** The workbooks the conversation may rely on. Absent in the settings panel, which only lists. */
  scope?: AnalysisScope;
  onScopeChange?: (scope: AnalysisScope) => void;
  /** A question is being answered: the selection it was asked with must not change under it. */
  scopeLocked?: boolean;
  /** Opens the knowledge management dialog; the link under the summary shows only when the
   * knowledge base took part in the pass. */
  onOpenKnowledge?: () => void;
  /** The suggestions, drawn at the end of the card once the folder has been read. Built by the
   * caller, which owns what a click on one of them does and which card shows them. */
  suggestions?: ReactNode;
}) {
  const { t } = useTranslation();
  const [error, setError] = useState<AppError | null>(null);
  const [busy, setBusy] = useState(false);
  const [inventory, setInventory] = useState<DataFolderReport | null>(null);
  const [confirmingReset, setConfirmingReset] = useState(false);
  const [resetting, setResetting] = useState(false);
  const showSuggestion = dataFolder === null && suggestedDataFolder !== null;
  const shownPath = (path: string) =>
    detail === "collapsible" ? abbreviateFolderPath(path) : path;
  const { finishedPasses, reset: resetIndexing } = indexing;
  const pending = inventory !== null && analysisPending(inventory.files);
  const pendingCount = inventory === null ? 0 : countPending(inventory.files);
  const chosen = scope === undefined ? [] : scopedPaths(scope);
  const selection = scope === undefined ? "none" : selectionOf(scope);
  const usableCount = inventory === null ? 0 : inventory.files.filter(isSelectable).length;

  /* The search field: typing a search selects the analysed files it finds. It is shown only where the
     listing doubles as the selection, and not while a question is being answered. */
  const selectable = scope !== undefined && onScopeChange !== undefined;
  const analysedFiles = inventory === null ? [] : inventory.files.filter(isSelectable);
  const finder = useFinderSearch({
    domain: "data",
    analysed: analysedFiles,
    enabled: selectable && !scopeLocked && dataFolder !== null,
    onSelect: (paths, analysed) => {
      if (scope !== undefined && onScopeChange !== undefined) {
        onScopeChange(scopeFromSearch(scope, paths, analysed, Date.now()));
      }
    },
  });
  /* An index written before the profile existed is said, never rebuilt: only Analyse fills it. */
  const profileStatus = useProfileStatus("data", dataFolder, finishedPasses);

  const refreshInventory = useCallback(() => {
    if (dataFolder === null) {
      setInventory(null);
      return;
    }
    // A failed read leaves the panel without counts rather than with wrong ones.
    dataFolderInventory().then(setInventory, () => setInventory(null));
  }, [dataFolder]);

  useEffect(refreshInventory, [refreshInventory]);

  /* Coming back to the window is when a spreadsheet exported from another program most likely
     arrived - the same reasoning, and the same cheap re-read, as the Documents Folder card. */
  useEffect(() => {
    if (dataFolder === null) {
      return;
    }
    window.addEventListener("focus", refreshInventory);
    return () => window.removeEventListener("focus", refreshInventory);
  }, [dataFolder, refreshInventory]);
  useEffect(() => {
    if (finishedPasses > 0) {
      refreshInventory();
    }
  }, [finishedPasses, refreshInventory]);

  /* What the last pass did. It opens with how many spreadsheets were read and where, because the
     pass is over in a moment - without that line, an Analyse that finishes before the spinner is
     seen reads as a button that did nothing. */
  const summary = indexing.summary;
  const passLines =
    summary === null
      ? []
      : [
          t("dataFolder.passDone", {
            count: counted(
              summary.scannedFiles,
              t("dataFolder.spreadsheetOne"),
              t("dataFolder.spreadsheetMany"),
            ),
          }),
          ...(summary.removedFiles.length > 0
            ? [
                `${counted(
                  summary.removedFiles.length,
                  t("dataFolder.removedOne"),
                  t("dataFolder.removedMany"),
                )}.`,
              ]
            : []),
          ...(summary.renamedFiles.length > 0
            ? [
                `${counted(
                  summary.renamedFiles.length,
                  t("workFolder.renamedOne"),
                  t("workFolder.renamedMany"),
                )}.`,
                ...summary.renamedFiles
                  .slice(0, MAX_RENAMES_LISTED)
                  .map(({ from, to }) => t("workFolder.renamedItem", { from, to })),
                ...(summary.renamedFiles.length > MAX_RENAMES_LISTED
                  ? [
                      t("workFolder.renamedMore", {
                        count: summary.renamedFiles.length - MAX_RENAMES_LISTED,
                      }),
                    ]
                  : []),
              ]
            : []),
          ...(summary.renameFailedFiles.length > 0
            ? [
                `${counted(
                  summary.renameFailedFiles.length,
                  t("workFolder.renameFailedOne"),
                  t("workFolder.renameFailedMany"),
                )}.`,
              ]
            : []),
          /* What the knowledge base read in the tables, counts only. */
          ...knowledgeLines(t, summary.knowledge, "data"),
        ];

  const passSummary =
    passLines.length === 0 ? null : (
      <>
        <p className="inventory-detail__summary">
          {passLines.map((line, index) => (
            <Fragment key={index}>
              {index > 0 ? <br /> : null}
              {line}
            </Fragment>
          ))}
        </p>
        {onOpenKnowledge !== undefined && summary?.knowledge ? (
          <KnowledgeLink onOpen={onOpenKnowledge} />
        ) : null}
      </>
    );

  /* A file here that is not a spreadsheet is red for a different reason than a workbook with no
     table in it, and the dot's words say which. */
  const statusLabel = (file: FileRecord) =>
    file.kind === "tabular_candidate"
      ? t(`dataFolder.status.${file.processingStatus}`)
      : t("dataFolder.status.notTabular");

  const reveal = async () => {
    setError(null);
    try {
      await revealDataFolder();
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    }
  };

  const revealFile = async (relativePath: string) => {
    setError(null);
    try {
      await revealDataFile(relativePath);
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    }
  };

  const confirmReset = async () => {
    setError(null);
    setResetting(true);
    try {
      await resetDataIndex();
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

  const run = async (action: typeof chooseDataFolder) => {
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
          {dataFolder !== null ? (
            <button
              type="button"
              className="button button--compact"
              disabled={busy || indexing.running || resetting}
              onClick={() => setConfirmingReset(true)}
            >
              {t("dataFolder.reset")}
            </button>
          ) : null}
          {dataFolder !== null ? (
            <AnalyseButton indexing={indexing} emphasised={pending} pendingCount={pendingCount} />
          ) : null}
        </div>
        {indexing.progress === null ? null : (
          <AnalysisProgress
            fraction={analysisFraction(indexing.progress)}
            label={
              indexing.progress.readingNames === true
                ? t("dataFolder.progressNamesLabel")
                : t("dataFolder.progressLabel")
            }
          />
        )}
      </div>
      {inventory === null ? (
        passSummary
      ) : (
        <>
          <p className="card__description">
            {/* The same three counts, in the same colours, as the Documents Folder card. */}
            {counted(inventory.summary.totalFiles, t("workFolder.filesOne"), t("workFolder.filesMany"))}
            {", "}
            {counted(
              inventory.summary.analysedFiles,
              t("workFolder.analysedOne"),
              t("workFolder.analysedMany"),
            )}{" "}
            <StatusDot tone="ok" label={t("dataFolder.status.indexed")} />
            {", "}
            {counted(
              inventory.summary.unreadableFiles,
              t("workFolder.unreadableOne"),
              t("workFolder.unreadableMany"),
            )}{" "}
            <StatusDot tone="fail" label={t("dataFolder.status.failed")} />.
          </p>
          {profileStatus === "needsAnalyse" ? (
            <p className="card__description finder__notice" role="status">
              {t("finder.needsAnalyse")}
            </p>
          ) : null}
          {inventory.summary.totalFiles === 0 ? (
            passSummary
          ) : detail === "collapsible" ? (
            <>
              {selectable ? (
                <FinderControls
                  search={finder}
                  analysed={analysedFiles}
                  placeholder={t("finder.placeholderData")}
                  disabled={scopeLocked}
                />
              ) : null}
            <details className="inventory-detail scope-picker">
              <summary className="disclosure">
                {selection === "some" ? (
                  t("dataFolder.scopeSummaryExplicit", {
                    files: counted(chosen.length, t("chat.scopeFileOne"), t("chat.scopeFileMany")),
                  })
                ) : (
                  <>
                    {t(selection === "all" ? "dataFolder.scopeSummaryAll" : "dataFolder.scopeSummaryNone")}
                    <span
                      className="warning-glyph__wrap"
                      title={t(
                        selection === "all"
                          ? "dataFolder.scopeWholeWarning"
                          : "dataFolder.scopeNoneWarning",
                      )}
                    >
                      <WarningGlyph />
                    </span>
                  </>
                )}
              </summary>
              {scope === undefined || onScopeChange === undefined ? null : (
                <SelectAll
                  scope={scope}
                  onScopeChange={onScopeChange}
                  disabled={scopeLocked || usableCount === 0}
                  label={t("dataFolder.scopeSelectAll")}
                />
              )}
              <FileList
                files={shownFiles(inventory.files, finder.found, finder.showAll)}
                selectionBase={inventory.files}
                statusLabel={statusLabel}
                scope={scope}
                onScopeChange={onScopeChange}
                scopeLocked={scopeLocked}
              />
              {passSummary}
            </details>
            </>
          ) : (
            <>
              {passSummary}
              <FileList
                files={inventory.files}
                statusLabel={statusLabel}
                onReveal={(path) => void revealFile(path)}
              />
            </>
          )}
        </>
      )}
      {error === null ? null : <ErrorBanner error={error} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
      {confirmingReset ? (
        <ConfirmDialog
          title={t("dataFolder.resetTitle")}
          body={t("dataFolder.resetBody")}
          confirmLabel={t("dataFolder.reset")}
          destructive
          busy={resetting}
          onConfirm={() => void confirmReset()}
          onCancel={() => setConfirmingReset(false)}
        />
      ) : null}
      {/* Outside the pass summary on purpose: that summary is empty after a pass that changed
          nothing, and sits inside a disclosure. */}
      {inventory === null ? null : suggestions}
    </section>
  );
}
