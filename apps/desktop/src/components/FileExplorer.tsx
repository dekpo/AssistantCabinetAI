import { useEffect, useId, useMemo, useRef, useState, type CSSProperties } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { unfinishedPercent } from "../lib/analysis";
import { selectionOf, tickedPaths, toggleScopeFile } from "../lib/analysisScope";
import { normaliseError, type AppError } from "../lib/errors";
import {
  activeFilterCount,
  activePills,
  folderChoices,
  formatChoices,
  leftByOthers,
  NO_CHOICES,
  reasonKeys,
  removePill,
  stateCounts,
  toggleChoice,
  withoutTypedFormat,
  withoutTypedFormats,
  type ActivePill,
  type FilterKind,
  type PanelChoices,
} from "../lib/explorerFilters";
import {
  countsByFamily,
  filterEntries,
  explorerKey,
  foundKeys,
  isTickable,
  mergeInventories,
  sortEntries,
  type ExplorerEntry,
  type ExplorerFilter,
  type Family,
} from "../lib/explorerModel";
import {
  addEntriesToScopes,
  clearedScopes,
  entriesToTick,
  isFiltering,
  selectionNotes,
  tickableRecords,
  usedCount,
} from "../lib/explorerSelection";
import { boxState, folderState, nothingToSearch } from "../lib/explorerStates";
import { ROW_HEIGHT, rowBudget } from "../lib/explorerWindow";
import {
  cancelAnalysis,
  dataFolderInventory,
  revealDataFile,
  revealWorkFile,
  workFolderInventory,
  type AnalysisScope,
  type ExplorerDensity,
  type FileRecord,
} from "../lib/ipc";
import { dataPassLines, documentPassLines } from "../lib/passSummary";
import { counted } from "../lib/plural";
import { useExplorerFolders } from "../state/useExplorerFolders";
import { useFinderSearch } from "../state/useFinderSearch";
import { useFolderInventory } from "../state/useFolderInventory";
import type { IndexingState } from "../state/useIndexing";
import { useNow } from "../state/useNow";
import { useProfileStatus } from "../state/useProfileStatus";
import { useVectorShare } from "../state/useVectorShare";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { ExplorerActivePills } from "./ExplorerActivePills";
import { ExplorerFilterPanel } from "./ExplorerFilterPanel";
import { ExplorerBottomBar } from "./ExplorerBottomBar";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { SearchGlyph } from "./ExplorerGlyphs";
import { ExplorerFamilySetup, ExplorerFirstStart } from "./ExplorerSetup";

/** The family filter of the segmented control: everything, or one family. */
type FamilyChoice = "all" | Family;

const FAMILY_CHOICES: readonly FamilyChoice[] = ["all", "documents", "tables"];

/**
 * The left panel: one compact box with the two families of files, replacing the two folder cards of
 * the sidebar. It lists every file of both folders under its family, by extension, flat and bounded;
 * the search field, the segmented control and the bottom bar narrow and select; the two Analyser
 * buttons start the two passes that already exist. Nothing here reads or writes a file: the
 * inventories, the finder and the passes are the ones the cards used, and the selection is still the
 * two scopes the chat is sent with (`docs/CLIENT.md`, "The left panel").
 */
export function FileExplorer({
  workFolder,
  dataFolder,
  suggestedWorkFolder,
  suggestedDataFolder,
  onWorkFolderChosen,
  onDataFolderChosen,
  indexing,
  dataIndexing,
  scope,
  dataScope,
  onScopeChange,
  onDataScopeChange,
  scopeLocked,
  density,
  folded,
  onFoldedChange,
  onOpenKnowledge,
  onOpenDocument,
  onOpenTable,
  refreshToken,
}: {
  workFolder: string | null;
  dataFolder: string | null;
  /** The folders the software proposes (`~/AssistantCabinetAI/Docs` and `/Data`), or null when none. */
  suggestedWorkFolder: string | null;
  suggestedDataFolder: string | null;
  /** A folder was chosen or made. The Documents one is the guarded change of `App` (unsaved notes
   * ask first); the Data one just saves the setting. */
  onWorkFolderChosen: (path: string) => void;
  onDataFolderChosen: (path: string) => void;
  /** The documents' pass and the workbooks' pass: two folders, two passes. */
  indexing: IndexingState;
  dataIndexing: IndexingState;
  /** The two lists the chat is sent with, one per folder. */
  scope: AnalysisScope;
  dataScope: AnalysisScope;
  onScopeChange: (scope: AnalysisScope) => void;
  onDataScopeChange: (scope: AnalysisScope) => void;
  /** A question is being answered: changing the selection under it would describe another answer. */
  scopeLocked: boolean;
  density: ExplorerDensity;
  folded: Record<Family, boolean>;
  onFoldedChange: (family: Family, folded: boolean) => void;
  onOpenKnowledge: () => void;
  onOpenDocument: (record: FileRecord) => void;
  onOpenTable: (record: FileRecord) => void;
  /** Counted up when the software itself wrote a file into the Documents folder (a saved note). */
  refreshToken: number;
}) {
  const { t } = useTranslation();
  const [familyChoice, setFamilyChoice] = useState<FamilyChoice>("all");
  const [error, setError] = useState<AppError | null>(null);
  /* The choices of the filter panel. The family is held apart: it is the segmented control, outside
     the panel. A filter only narrows the view; none of this state is the selection. */
  const [choices, setChoices] = useState<PanelChoices>(NO_CHOICES);
  const [panelOpen, setPanelOpen] = useState(false);
  /* Clearing a family's analysis asks first; the question names the family. */
  const [confirmingClear, setConfirmingClear] = useState<Family | null>(null);
  const folders = useExplorerFolders({
    indexing,
    dataIndexing,
    onWorkFolderChosen,
    onDataFolderChosen,
    onError: setError,
  });
  /* One clock for both families: it ticks only while a pass runs. */
  const now = useNow(indexing.running || dataIndexing.running);
  const panelId = useId();
  const filtersButton = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);

  const documentsInventory = useFolderInventory(
    workFolderInventory,
    workFolder,
    indexing.finishedPasses,
    refreshToken + folders.clears.documents,
  );
  const dataInventory = useFolderInventory(
    dataFolderInventory,
    dataFolder,
    dataIndexing.finishedPasses,
    folders.clears.tables,
  );
  const merged = useMemo(
    () =>
      mergeInventories(documentsInventory?.files ?? [], dataInventory?.files ?? [], {
        documents: workFolder,
        data: dataFolder,
      }),
    [documentsInventory, dataInventory, workFolder, dataFolder],
  );
  const entries = merged.entries;

  /* One field, two searches: the finder works per folder, so the same text is asked of each and the
     two answers are joined by (folder, path). A search that could not run shows everything and says
     so, rather than hiding one folder's files behind the other's answer. */
  const documentsFinder = useFinderSearch({
    domain: "documents",
    folder: workFolder,
    finishedPasses: indexing.finishedPasses,
    inventory: documentsInventory,
    enabled: !scopeLocked && workFolder !== null,
  });
  const dataFinder = useFinderSearch({
    domain: "data",
    folder: dataFolder,
    finishedPasses: dataIndexing.finishedPasses,
    inventory: dataInventory,
    enabled: !scopeLocked && dataFolder !== null,
  });
  const text = documentsFinder.text;
  const searchFailed = documentsFinder.failed || dataFinder.failed;
  const found = useMemo(
    () => (searchFailed ? null : foundKeys(documentsFinder.found, dataFinder.found)),
    [searchFailed, documentsFinder.found, dataFinder.found],
  );
  const setText = (next: string) => {
    documentsFinder.setText(next);
    dataFinder.setText(next);
  };

  /* What the finders read out of the text as extensions: they light the matching format pills. */
  const typedFormats = useMemo(
    () =>
      searchFailed || found === null
        ? []
        : [...new Set([...documentsFinder.typedFormats, ...dataFinder.typedFormats])],
    [searchFailed, found, documentsFinder.typedFormats, dataFinder.typedFormats],
  );
  const filter: ExplorerFilter = {
    family: familyChoice === "all" ? null : familyChoice,
    formats: choices.formats,
    states: choices.states,
    folders: choices.folders,
    found,
  };
  const searched = useMemo(() => filterEntries(entries, { found }), [entries, found]);
  const shown = useMemo(
    () => filterEntries(entries, filter),
    [entries, familyChoice, choices, found],
  );
  const listed = useMemo(
    () => ({
      documents: sortEntries(
        shown.filter((entry) => entry.family === "documents"),
        "name",
      ),
      tables: sortEntries(
        shown.filter((entry) => entry.family === "tables"),
        "name",
      ),
    }),
    [shown],
  );
  const counts = useMemo(() => countsByFamily(entries), [entries]);
  /* The number beside a segment is what the other filters leave for it: the family left out. */
  const forSegments = leftByOthers(entries, filter, "family");
  const segmentCounts: Record<FamilyChoice, number> = {
    all: forSegments.length,
    documents: forSegments.filter((entry) => entry.family === "documents").length,
    tables: forSegments.filter((entry) => entry.family === "tables").length,
  };
  const pills = activePills(filter, typedFormats);
  /* The folder choices are worked out only when something shows them: the open panel, or a folder pill
     (which says the root when both roots hold a folder of that name). */
  const folderOptions =
    panelOpen || choices.folders.size > 0 ? folderChoices(entries, filter) : [];

  const toggleFilter = (kind: FilterKind, value: string) =>
    setChoices((now) => toggleChoice(now, kind, value));
  const removeTyped = (extension: string) => setText(withoutTypedFormat(text, extension));
  const removeActivePill = (pill: ActivePill) => {
    if (pill.typed) {
      removeTyped(pill.value);
    } else {
      setChoices((now) => removePill(now, pill));
    }
  };
  const clearFilters = () => {
    setChoices(NO_CHOICES);
    if (typedFormats.length > 0) {
      setText(withoutTypedFormats(text, typedFormats));
    }
  };
  const closePanel = (returnFocus: boolean) => {
    setPanelOpen(false);
    if (returnFocus) {
      filtersButton.current?.focus();
    }
  };
  /* Escape closes the panel and gives the focus back to its button; a click anywhere else closes it
     and leaves the focus where the click put it. */
  useEffect(() => {
    if (!panelOpen) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closePanel(true);
      }
    };
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!panel.current?.contains(target) && !filtersButton.current?.contains(target)) {
        closePanel(false);
      }
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPointer);
    };
  }, [panelOpen]);

  /* Why each file was found, from both finders, by (folder, path). Only while a text search is active. */
  const reasonsByKey = useMemo(() => {
    const map = new Map<string, string[]>();
    if (found === null) {
      return map;
    }
    for (const [root, finder] of [
      ["documents", documentsFinder],
      ["data", dataFinder],
    ] as const) {
      for (const [path, reasons] of Object.entries(finder.reasons)) {
        map.set(explorerKey(root, path), reasonKeys(reasons));
      }
    }
    return map;
  }, [found, documentsFinder.reasons, dataFinder.reasons]);
  const reasonOf = (entry: ExplorerEntry) => {
    const keys = reasonsByKey.get(entry.key) ?? [];
    return keys.length === 0
      ? undefined
      : { text: t(keys[0] ?? ""), title: keys.map((key) => t(key)).join(", ") };
  };
  /* Which situation the box is in, from the same two inventories: nothing chosen yet, folders chosen
     but empty, or in use. Until a folder holds a file there is nothing to search or to filter. */
  const documentsState = folderState({
    folder: workFolder,
    loaded: documentsInventory !== null,
    folderFiles: documentsInventory?.files.length ?? 0,
    familyFiles: counts.documents.total,
  });
  const tablesState = folderState({
    folder: dataFolder,
    loaded: dataInventory !== null,
    folderFiles: dataInventory?.files.length ?? 0,
    familyFiles: counts.tables.total,
  });
  const box = boxState(documentsState, tablesState);
  const idle = nothingToSearch(box);

  /* The selection: the same two scopes the chat is sent with. A row's tick, the two links of the
     bottom bar and the counts all read them through the analysed files of each folder. */
  const tickable = {
    documents: tickableRecords(entries, "documents"),
    data: tickableRecords(entries, "data"),
  };
  const tickedPathsByRoot = {
    documents: new Set(tickedPaths(scope, tickable.documents)),
    data: new Set(tickedPaths(dataScope, tickable.data)),
  };
  const isTicked = (entry: ExplorerEntry) =>
    isTickable(entry) && tickedPathsByRoot[entry.root].has(entry.path);
  const toggleTick = (entry: ExplorerEntry) => {
    if (!isTickable(entry)) {
      return;
    }
    const now = Date.now();
    if (entry.root === "documents") {
      onScopeChange(toggleScopeFile(scope, entry.record, tickable.documents, now));
    } else {
      onDataScopeChange(toggleScopeFile(dataScope, entry.record, tickable.data, now));
    }
  };
  const scopes = { documents: scope, data: dataScope };
  const toTick = isFiltering(filter) ? entriesToTick(shown, entries, scopes) : [];
  const documentsUsed = usedCount(scope, tickable.documents);
  const tablesUsed = usedCount(dataScope, tickable.data);

  const budget = rowBudget({ documents: !folded.documents, tables: !folded.tables });
  const rowHeight = ROW_HEIGHT[density];

  const documentsProfile = useProfileStatus("documents", workFolder, indexing.finishedPasses);
  const dataProfile = useProfileStatus("data", dataFolder, dataIndexing.finishedPasses);
  const vectors = useVectorShare(workFolder, indexing.finishedPasses);

  const reveal = async (entry: ExplorerEntry) => {
    setError(null);
    try {
      await (entry.root === "documents" ? revealWorkFile(entry.path) : revealDataFile(entry.path));
    } catch (raw: unknown) {
      setError(normaliseError(raw));
    }
  };
  const open = (entry: ExplorerEntry) =>
    entry.root === "documents" ? onOpenDocument(entry.record) : onOpenTable(entry.record);

  const query = text.trim();
  const notAnalysed = searched.filter((entry) => !isTickable(entry)).length;
  const familyLabels: Record<FamilyChoice, string> = {
    all: t("explorer.segmentAll"),
    documents: t("explorer.familyDocuments"),
    tables: t("explorer.familyTables"),
  };

  return (
    <section
      className="explorer"
      aria-label={t("explorer.title")}
      style={{ "--explorer-row": `${rowHeight}px` } as CSSProperties}
    >
      <header className="explorer__head">
        <div className="explorer__titleline">
          <h2 className="explorer__title">{t("explorer.title")}</h2>
          {/* The saved views arrive with the filters of a later step: the switch is drawn, and the
              second choice waits. */}
          <div className="explorer__views" role="group" aria-label={t("explorer.viewSwitch")}>
            <button type="button" className="explorer__view" aria-pressed="true">
              {t("explorer.viewFiles")}
            </button>
            <button
              type="button"
              className="explorer__view"
              aria-pressed="false"
              disabled
              title={t("explorer.soon")}
            >
              {t("explorer.viewViews")}
            </button>
          </div>
        </div>
        <div className="explorer__searchline">
          <label className="explorer__search">
            <SearchGlyph />
            <input
              type="search"
              className="explorer__search-input"
              value={text}
              placeholder={t("explorer.searchPlaceholder")}
              aria-label={t("explorer.searchLabel")}
              disabled={scopeLocked || idle}
              spellCheck={false}
              autoComplete="off"
              onChange={(event) => setText(event.target.value)}
            />
          </label>
          <button
            ref={filtersButton}
            type="button"
            className="button button--compact"
            aria-expanded={panelOpen}
            aria-controls={panelOpen ? panelId : undefined}
            disabled={idle}
            onClick={() => setPanelOpen((open) => !open)}
          >
            {t("explorer.filters")}
            {pills.length === 0 ? null : (
              <span
                className="explorer__filters-badge"
                aria-label={t("explorer.filtersActive", { count: activeFilterCount(filter, typedFormats) })}
              >
                {activeFilterCount(filter, typedFormats)}
              </span>
            )}
          </button>
        </div>
        {panelOpen && !idle ? (
          <ExplorerFilterPanel
            id={panelId}
            panelRef={panel}
            formats={formatChoices(entries, filter, typedFormats)}
            chosenFormats={choices.formats}
            typedFormats={typedFormats}
            stateCounts={stateCounts(entries, filter)}
            chosenStates={choices.states}
            folders={folderOptions}
            chosenFolders={choices.folders}
            disabled={idle}
            anyActive={pills.length > 0}
            onToggle={toggleFilter}
            onRemoveTyped={removeTyped}
            onClear={clearFilters}
            onClose={() => closePanel(true)}
          />
        ) : null}
        {idle ? null : (
          <div className="explorer__segments" role="group" aria-label={t("explorer.segmentLabel")}>
            {FAMILY_CHOICES.map((choice) => (
              <button
                key={choice}
                type="button"
                className="explorer__segment"
                aria-pressed={familyChoice === choice}
                onClick={() => setFamilyChoice(choice)}
              >
                {familyLabels[choice]}
                <small className="explorer__segment-count">{segmentCounts[choice]}</small>
              </button>
            ))}
          </div>
        )}
        <ExplorerActivePills
          pills={pills}
          folders={folderOptions}
          onRemove={removeActivePill}
          onClear={clearFilters}
        />
        {searchFailed ? (
          <p className="explorer__status" role="status">
            {t("finder.failed")}
          </p>
        ) : found !== null && query !== "" ? (
          <p className="explorer__status" role="status">
            {searched.length === 0
              ? t("finder.none", { query })
              : t("finder.found", {
                  files: counted(searched.length, t("finder.foundOne"), t("finder.foundMany")),
                  query,
                })}
            {notAnalysed > 0
              ? ` ${t("finder.notAnalysed", {
                  count: counted(
                    notAnalysed,
                    t("finder.notAnalysedOne"),
                    t("finder.notAnalysedMany"),
                  ),
                })}`
              : ""}
          </p>
        ) : null}
      </header>

      {box === "firstStart" ? (
        <ExplorerFirstStart
          busy={folders.busy}
          canCreate={suggestedWorkFolder !== null || suggestedDataFolder !== null}
          onCreateBoth={() => void folders.createBoth()}
        />
      ) : null}

      {workFolder === null ? (
        <ExplorerFamilySetup
          family="documents"
          suggested={suggestedWorkFolder}
          busy={folders.busy}
          onCreate={() => void folders.createSuggested("documents")}
          onChoose={() => void folders.choose("documents")}
        />
      ) : (
        <ExplorerFamilySection
          family="documents"
          folder={workFolder}
          isEmpty={documentsState === "empty"}
          folderActions={{
            reveal: () => void folders.reveal("documents"),
            change: () => void folders.choose("documents"),
            clear: () => setConfirmingClear("documents"),
          }}
          counts={counts.documents}
          entries={listed.documents}
          allEntries={entries}
          open={!folded.documents}
          onToggleOpen={() => onFoldedChange("documents", !folded.documents)}
          rows={budget.documents}
          rowHeight={rowHeight}
          indexing={indexing}
          now={now}
          onStop={() => void cancelAnalysis("documents")}
          locked={scopeLocked}
          isTicked={isTicked}
          onToggleTick={toggleTick}
          onOpenFile={open}
          onRevealFile={(entry) => void reveal(entry)}
          needsAnalyse={documentsProfile === "needsAnalyse"}
          meaningPercent={unfinishedPercent(vectors)}
          passLines={documentPassLines(t, indexing.summary)}
          showKnowledgeLink={indexing.summary?.knowledge != null}
          onOpenKnowledge={onOpenKnowledge}
          filtering={isFiltering(filter)}
          reasonOf={reasonOf}
        />
      )}
      {dataFolder === null ? (
        <ExplorerFamilySetup
          family="tables"
          suggested={suggestedDataFolder}
          busy={folders.busy}
          onCreate={() => void folders.createSuggested("tables")}
          onChoose={() => void folders.choose("tables")}
        />
      ) : (
        <ExplorerFamilySection
          family="tables"
          folder={dataFolder}
          isEmpty={tablesState === "empty"}
          folderActions={{
            reveal: () => void folders.reveal("tables"),
            change: () => void folders.choose("tables"),
            clear: () => setConfirmingClear("tables"),
          }}
          counts={counts.tables}
          entries={listed.tables}
          allEntries={entries}
          open={!folded.tables}
          onToggleOpen={() => onFoldedChange("tables", !folded.tables)}
          rows={budget.tables}
          rowHeight={rowHeight}
          indexing={dataIndexing}
          now={now}
          onStop={() => void cancelAnalysis("data")}
          locked={scopeLocked}
          isTicked={isTicked}
          onToggleTick={toggleTick}
          onOpenFile={open}
          onRevealFile={(entry) => void reveal(entry)}
          needsAnalyse={dataProfile === "needsAnalyse"}
          meaningPercent={null}
          passLines={dataPassLines(t, dataIndexing.summary)}
          showKnowledgeLink={dataIndexing.summary?.knowledge != null}
          onOpenKnowledge={onOpenKnowledge}
          filtering={isFiltering(filter)}
          reasonOf={reasonOf}
        />
      )}

      {error === null ? null : <ErrorBanner error={error} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
      {dataIndexing.error === null ? null : <ErrorBanner error={dataIndexing.error} />}

      {idle ? null : (
        <ExplorerBottomBar
          documentsUsed={documentsUsed}
          tablesUsed={tablesUsed}
          toTick={toTick.length}
          onTick={() => {
            const next = addEntriesToScopes(scopes, toTick, entries, Date.now());
            onScopeChange(next.documents);
            onDataScopeChange(next.data);
          }}
          canClear={selectionOf(scope) !== "none" || selectionOf(dataScope) !== "none"}
          onClear={() => {
            const next = clearedScopes(Date.now());
            onScopeChange(next.documents);
            onDataScopeChange(next.data);
          }}
          locked={scopeLocked}
          notes={selectionNotes(selectionOf(scope), selectionOf(dataScope))}
          unsupported={merged.unsupported}
        />
      )}
      {confirmingClear === null ? null : (
        <ConfirmDialog
          title={t(confirmingClear === "documents" ? "workFolder.resetTitle" : "dataFolder.resetTitle")}
          body={t(confirmingClear === "documents" ? "workFolder.resetBody" : "dataFolder.resetBody")}
          confirmLabel={t("explorer.menuClear")}
          destructive
          busy={folders.resetting}
          onConfirm={() => void folders.clear(confirmingClear).then(() => setConfirmingClear(null))}
          onCancel={() => setConfirmingClear(null)}
        />
      )}
    </section>
  );
}
