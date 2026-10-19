import { useMemo, useState, type CSSProperties } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { unfinishedPercent } from "../lib/analysis";
import { selectionOf, tickedPaths, toggleScopeFile } from "../lib/analysisScope";
import { normaliseError, type AppError } from "../lib/errors";
import {
  countsByFamily,
  filterEntries,
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
import { useFinderSearch } from "../state/useFinderSearch";
import { useFolderInventory } from "../state/useFolderInventory";
import type { IndexingState } from "../state/useIndexing";
import { useProfileStatus } from "../state/useProfileStatus";
import { useVectorShare } from "../state/useVectorShare";
import { ErrorBanner } from "./ErrorBanner";
import { ExplorerBottomBar } from "./ExplorerBottomBar";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { SearchGlyph } from "./ExplorerGlyphs";

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

  const documentsInventory = useFolderInventory(
    workFolderInventory,
    workFolder,
    indexing.finishedPasses,
    refreshToken,
  );
  const dataInventory = useFolderInventory(
    dataFolderInventory,
    dataFolder,
    dataIndexing.finishedPasses,
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

  const filter: ExplorerFilter = {
    family: familyChoice === "all" ? null : familyChoice,
    found,
  };
  const searched = useMemo(() => filterEntries(entries, { found }), [entries, found]);
  const shown = useMemo(() => filterEntries(entries, filter), [entries, filter.family, found]);
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
  const segmentCounts: Record<FamilyChoice, number> = {
    all: searched.length,
    documents: searched.filter((entry) => entry.family === "documents").length,
    tables: searched.filter((entry) => entry.family === "tables").length,
  };

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
              disabled={scopeLocked}
              spellCheck={false}
              autoComplete="off"
              onChange={(event) => setText(event.target.value)}
            />
          </label>
          {/* The filter panel is the next step: the button is drawn and waits. */}
          <button
            type="button"
            className="button button--compact"
            disabled
            title={t("explorer.soon")}
          >
            {t("explorer.filters")}
          </button>
        </div>
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

      <ExplorerFamilySection
        family="documents"
        folder={workFolder}
        counts={counts.documents}
        entries={listed.documents}
        allEntries={entries}
        open={!folded.documents}
        onToggleOpen={() => onFoldedChange("documents", !folded.documents)}
        rows={budget.documents}
        rowHeight={rowHeight}
        indexing={indexing}
        onStop={() => void cancelAnalysis()}
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
      />
      <ExplorerFamilySection
        family="tables"
        folder={dataFolder}
        counts={counts.tables}
        entries={listed.tables}
        allEntries={entries}
        open={!folded.tables}
        onToggleOpen={() => onFoldedChange("tables", !folded.tables)}
        rows={budget.tables}
        rowHeight={rowHeight}
        indexing={dataIndexing}
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
      />

      {error === null ? null : <ErrorBanner error={error} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
      {dataIndexing.error === null ? null : <ErrorBanner error={dataIndexing.error} />}

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
    </section>
  );
}
