import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type Dispatch,
  type SetStateAction,
} from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { unfinishedPercent } from "../lib/analysis";
import { selectionOf, toggleScopeFile } from "../lib/analysisScope";
import { restoreCount, showsTickLink, targetOf, unreachableCount } from "../lib/autoSelect";
import { normaliseError, type AppError } from "../lib/errors";
import { inventoryRefresh } from "../lib/inventoryRefresh";
import { attachFacts, type EntityKind } from "../lib/explorerFacts";
import {
  activeFilterCount,
  activePills,
  entityOptions,
  folderChoices,
  formatChoices,
  leftByOthers,
  NO_CHOICES,
  reasonKeys,
  removePill,
  setEntityRole,
  stateCounts,
  toggleChoice,
  typeChoices,
  withoutTypedFormat,
  withoutTypedFormats,
  yearChoices,
  type ActivePill,
  type FilterKind,
  type PanelChoices,
} from "../lib/explorerFilters";
import {
  countsByFamily,
  filterEntries,
  explorerKeyOfIndexKey,
  foundKeys,
  isTickable,
  mergeInventories,
  folderKey,
  listSort,
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
  scopeFamilyOf,
  scopeRecord,
  selectionNotes,
  tickableRecords,
  tickedKeysOf,
  usedCount,
  type ScopesByFamily,
} from "../lib/explorerSelection";
import { boxState, folderState, nothingToSearch } from "../lib/explorerStates";
import {
  addView,
  deleteView,
  missingEntities,
  newViewId,
  renameView,
  savedFromView,
} from "../lib/explorerViews";
import { ROW_HEIGHT, rowBudget } from "../lib/explorerWindow";
import type { ExplorerView, FamilyChoice } from "../lib/explorerView";
import {
  cancelAnalysis,
  dataFolderInventory,
  revealDataFile,
  revealWorkFile,
  workFolderInventory,
  type AnalysisScope,
  type ExplorerDensity,
  type FileRecord,
  type SavedViewSetting,
} from "../lib/ipc";
import type { ExplorerFacts } from "../lib/overviewModel";
import { counted } from "../lib/plural";
import { useAutoSelect } from "../state/useAutoSelect";
import { useExplorerFolders } from "../state/useExplorerFolders";
import { useFinderSearch } from "../state/useFinderSearch";
import { useFolderInventory } from "../state/useFolderInventory";
import type { IndexingState } from "../state/useIndexing";
import { useKnowledgeFacts } from "../state/useKnowledgeFacts";
import { useNow } from "../state/useNow";
import { useProfileStatus } from "../state/useProfileStatus";
import { useVectorShare } from "../state/useVectorShare";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { ExplorerActivePills } from "./ExplorerActivePills";
import { ExplorerAutoSwitch } from "./ExplorerAutoSwitch";
import { ExplorerKnowledgeFilters } from "./ExplorerKnowledgeFilters";
import { ExplorerViewsPanel } from "./ExplorerViewsPanel";
import { ExplorerFilterPanel } from "./ExplorerFilterPanel";
import { ExplorerBottomBar } from "./ExplorerBottomBar";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { SearchGlyph } from "./ExplorerGlyphs";
import { ExplorerFamilySetup, ExplorerFirstStart } from "./ExplorerSetup";

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
  folderError,
  indexing,
  dataIndexing,
  scope,
  dataScope,
  onScopeChange,
  onDataScopeChange,
  scopeLocked,
  density,
  folded,
  notesFolder,
  onFoldedChange,
  onOpenDocument,
  onOpenTable,
  refreshToken,
  view,
  onViewChange,
  searchClears,
  onFacts,
  onOpenReport,
  savedViews,
  onSavedViewsChange,
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
  /** A folder just chosen was refused when the settings were saved (the two folders overlap): the box
   * is where it was chosen, so it says so here too. */
  folderError?: AppError | null;
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
  /** The name of the Notes folder inside the Documents folder (a setting). */
  notesFolder: string;
  onFoldedChange: (family: Family, folded: boolean) => void;
  onOpenDocument: (record: FileRecord) => void;
  onOpenTable: (record: FileRecord) => void;
  /** Counted up when the software itself wrote a file into the Documents folder (a saved note). */
  refreshToken: number;
  /** The family of the segmented control and the choices of the filter panel. Held by `App`, so the
   * centre's overview can set them too. A filter only narrows the view; none of this is the
   * selection. */
  view: ExplorerView;
  onViewChange: Dispatch<SetStateAction<ExplorerView>>;
  /** Counted up when something outside asks for the typed search to be emptied. */
  searchClears: number;
  /** What the overview of the centre needs from the inventories read here. */
  onFacts: (facts: ExplorerFacts) => void;
  /** « Voir le rapport »: opens the centre on its overview. */
  onOpenReport: () => void;
  /** The views the user saved: filters under a name, kept in the settings file. */
  savedViews: readonly SavedViewSetting[];
  onSavedViewsChange: (views: SavedViewSetting[]) => void;
}) {
  const { t } = useTranslation();
  const familyChoice = view.family;
  const choices = view.choices;
  const setFamilyChoice = (family: FamilyChoice) =>
    onViewChange((current) => ({ ...current, family }));
  const setChoices = (change: PanelChoices | ((now: PanelChoices) => PanelChoices)) =>
    onViewChange((current) => ({
      ...current,
      choices: typeof change === "function" ? change(current.choices) : change,
    }));
  const [error, setError] = useState<AppError | null>(null);
  const [panelOpen, setPanelOpen] = useState(false);
  /* Files or views: the second shows the application's views and the user's own. */
  const [mode, setMode] = useState<"files" | "views">("files");
  /* Counted up each time the panel or the views open: the knowledge base is read again then. */
  const [openings, setOpenings] = useState(0);
  const [deletingView, setDeletingView] = useState<string | null>(null);
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

  /* A pass of one family changes what both inventories say (a workbook of the Documents folder is read
     by the tabular pass), so both are read again after any pass or clear (`lib/inventoryRefresh.ts`). */
  const refresh = inventoryRefresh({
    documentsPasses: indexing.finishedPasses,
    dataPasses: dataIndexing.finishedPasses,
    notesToken: refreshToken,
    documentsClears: folders.clears.documents,
    tablesClears: folders.clears.tables,
  });
  const documentsInventory = useFolderInventory(
    workFolderInventory,
    workFolder,
    refresh.passes,
    refresh.token,
  );
  const dataInventory = useFolderInventory(
    dataFolderInventory,
    dataFolder,
    refresh.passes,
    refresh.token,
  );
  const merged = useMemo(
    () =>
      mergeInventories(documentsInventory?.files ?? [], dataInventory?.files ?? [], {
        documents: workFolder,
        data: dataFolder,
      }),
    [documentsInventory, dataInventory, workFolder, dataFolder],
  );
  /* What the knowledge base knows of the files is read only when something shows or uses it: the open
     panel, the views, or a choice of the knowledge base already in force. */
  const knowledgeChosen =
    choices.people.size + choices.organisations.size + choices.types.size + choices.years.size > 0;
  const knowledge = useKnowledgeFacts({
    documentsFolder: workFolder,
    dataFolder,
    documentsPasses: indexing.finishedPasses,
    dataPasses: dataIndexing.finishedPasses,
    wanted: panelOpen || mode === "views" || knowledgeChosen,
    refresh: openings,
  });
  const entries = useMemo(
    () => attachFacts(merged.entries, knowledge.facts),
    [merged.entries, knowledge.facts],
  );

  /* One field, two searches: the finder works per family across both folders (a workbook found in the
     Documents folder is searched with the tables), so the same text is asked of each and the two
     answers, whose hits are index keys, are joined by the folder each file sits in. A search that could
     not run shows everything and says so, rather than hiding one family's files behind the other's
     answer. */
  const bothFolders = `${workFolder ?? ""}|${dataFolder ?? ""}`;
  const bothPasses = indexing.finishedPasses + dataIndexing.finishedPasses;
  const bothInventories = useMemo(
    () => [documentsInventory, dataInventory],
    [documentsInventory, dataInventory],
  );
  const documentsFinder = useFinderSearch({
    domain: "documents",
    folder: bothFolders,
    finishedPasses: bothPasses,
    inventory: bothInventories,
    enabled: !scopeLocked && workFolder !== null,
  });
  const dataFinder = useFinderSearch({
    domain: "data",
    folder: bothFolders,
    finishedPasses: bothPasses,
    inventory: bothInventories,
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
  /* The overview's link shows the unreadable files: a typed search would hide some of them. */
  useEffect(() => {
    if (searchClears > 0) {
      setText("");
    }
  }, [searchClears]);

  /* What the finders read out of the text as extensions: they light the matching format pills. */
  const typedFormats = useMemo(
    () =>
      searchFailed || found === null
        ? []
        : [...new Set([...documentsFinder.typedFormats, ...dataFinder.typedFormats])],
    [searchFailed, found, documentsFinder.typedFormats, dataFinder.typedFormats],
  );
  /* « Dans la conversation »: the files the conversation uses now, as keys. Worked out only while that
     filter is on. */
  const tickedKeys = useMemo(() => {
    if (!choices.inConversation) {
      return null;
    }
    const keys = new Set<string>();
    const ticked = tickedKeysOf({ documents: scope, data: dataScope }, entries);
    for (const family of ["documents", "data"] as const) {
      for (const key of ticked[family]) {
        keys.add(explorerKeyOfIndexKey(family, key));
      }
    }
    return keys;
  }, [choices.inConversation, entries, scope, dataScope]);
  const filter: ExplorerFilter = {
    family: familyChoice === "all" ? null : familyChoice,
    formats: choices.formats,
    states: choices.states,
    folders: choices.folders,
    people: choices.people,
    organisations: choices.organisations,
    types: choices.types,
    years: choices.years,
    found,
    ticked: tickedKeys,
  };
  const switches = { recent: choices.recent, conversation: choices.inConversation };
  const searched = useMemo(() => filterEntries(entries, { found }), [entries, found]);
  const shown = useMemo(
    () => filterEntries(entries, filter),
    [entries, familyChoice, choices, found, tickedKeys],
  );
  /* The Notes folder, as an origin-folder choice: one click shows the notes the user wrote. */
  const notesKey = notesFolder.trim() === "" ? null : folderKey("documents", notesFolder);
  const notesCount = useMemo(
    () =>
      notesKey === null ? 0 : filterEntries(entries, { folders: new Set([notesKey]) }).length,
    [entries, notesKey],
  );
  /* While a text search is active the lists follow the finder's ranking, so the file looked for is
     not the last of the list; the position is the one each finder gave its own folder's files. */
  const rankByKey = useMemo(() => {
    const rank = new Map<string, number>();
    for (const [family, finder] of [
      ["documents", documentsFinder],
      ["data", dataFinder],
    ] as const) {
      (finder.found ?? []).forEach((key, position) =>
        rank.set(explorerKeyOfIndexKey(family, key), position),
      );
    }
    return rank;
  }, [documentsFinder.found, dataFinder.found]);
  const sort = listSort({
    searching: found !== null,
    folders: choices.folders,
    notesKey,
    recent: choices.recent,
  });
  const listed = useMemo(
    () => ({
      documents: sortEntries(
        shown.filter((entry) => entry.family === "documents"),
        sort,
        rankByKey,
      ),
      tables: sortEntries(
        shown.filter((entry) => entry.family === "tables"),
        sort,
        rankByKey,
      ),
    }),
    [shown, sort, rankByKey],
  );
  const counts = useMemo(() => countsByFamily(entries), [entries]);
  /* The number beside a segment is what the other filters leave for it: the family left out. */
  const forSegments = leftByOthers(entries, filter, "family");
  const segmentCounts: Record<FamilyChoice, number> = {
    all: forSegments.length,
    documents: forSegments.filter((entry) => entry.family === "documents").length,
    tables: forSegments.filter((entry) => entry.family === "tables").length,
  };
  const pills = activePills(filter, typedFormats, switches);
  /* The folder choices are worked out only when something shows them: the open panel, or a folder pill
     (which says the root when both roots hold a folder of that name). */
  const folderOptions =
    panelOpen || choices.folders.size > 0 ? folderChoices(entries, filter) : [];

  const toggleFilter = (kind: FilterKind, value: string) =>
    setChoices((now) => toggleChoice(now, kind, value));
  const chooseRole = (kind: EntityKind, entityId: number, role: string | null) =>
    setChoices((now) => setEntityRole(now, kind, entityId, role));
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
    for (const [family, finder] of [
      ["documents", documentsFinder],
      ["data", dataFinder],
    ] as const) {
      for (const [key, reasons] of Object.entries(finder.reasons)) {
        map.set(explorerKeyOfIndexKey(family, key), reasonKeys(reasons));
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

  /* « Select. auto »: the selection follows the result of the filters, once the result has arrived.
     The rules are the pure state machine of `lib/autoSelect.ts`; this only feeds it and applies what
     it asks for. Off at every launch, locked while a question is being answered. */
  const auto = useAutoSelect({
    locked: scopeLocked,
    apply: (next: ScopesByFamily) => {
      onScopeChange(next.documents);
      onDataScopeChange(next.data);
    },
  });
  useEffect(() => {
    if (auto.state.on) {
      auto.send({ type: "resultArrived", target: targetOf(shown, entries, Date.now()) });
    }
  }, [auto.state.on, shown, entries]);

  /* The selection: the same two scopes the chat is sent with, one per family (the documents wherever
     they sit, the tables wherever they sit), each entry under the key the index knows the file by. A
     row's tick, the two links of the bottom bar and the counts all read them through the analysed
     files of each family. */
  const tickable = {
    documents: tickableRecords(entries, "documents"),
    data: tickableRecords(entries, "data"),
  };
  const scopes: ScopesByFamily = { documents: scope, data: dataScope };
  const tickedByFamily = tickedKeysOf(scopes, entries);
  const isTicked = (entry: ExplorerEntry) =>
    isTickable(entry) && tickedByFamily[scopeFamilyOf(entry)].has(scopeRecord(entry).relativePath);
  const toggleTick = (entry: ExplorerEntry) => {
    if (!isTickable(entry)) {
      return;
    }
    /* A tick by hand turns the automatic selection off and drops nothing else. */
    auto.send({ type: "manualChange" });
    const now = Date.now();
    const record = scopeRecord(entry);
    if (scopeFamilyOf(entry) === "documents") {
      onScopeChange(toggleScopeFile(scope, record, tickable.documents, now));
    } else {
      onDataScopeChange(toggleScopeFile(dataScope, record, tickable.data, now));
    }
  };
  const toTick = isFiltering(filter) ? entriesToTick(shown, entries, scopes) : [];
  const documentsUsed = usedCount(scope, tickable.documents);
  const tablesUsed = usedCount(dataScope, tickable.data);

  const budget = rowBudget({ documents: !folded.documents, tables: !folded.tables });
  const rowHeight = ROW_HEIGHT[density];

  const documentsProfile = useProfileStatus("documents", workFolder, indexing.finishedPasses);
  const dataProfile = useProfileStatus("data", dataFolder, dataIndexing.finishedPasses);
  const vectors = useVectorShare(workFolder, indexing.finishedPasses);
  const meaningPercent = unfinishedPercent(vectors);
  const documentsNeedsAnalyse = documentsProfile === "needsAnalyse";
  const tablesNeedsAnalyse = dataProfile === "needsAnalyse";
  /* What the overview of the centre reads from here, reported when one of them changes. */
  const facts = useMemo<ExplorerFacts>(
    () => ({
      entries,
      documentsState,
      tablesState,
      meaningPercent,
      documentsNeedsAnalyse,
      tablesNeedsAnalyse,
    }),
    [entries, documentsState, tablesState, meaningPercent, documentsNeedsAnalyse, tablesNeedsAnalyse],
  );
  useEffect(() => onFacts(facts), [facts, onFacts]);

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
  /* A saved view opens as the filters it holds, in the files side: the typed text, the lists and the
     selection are left as they are. */
  const openView = (next: ExplorerView) => {
    onViewChange(next);
    setMode("files");
  };
  /* The people and organisations of the filters that the files no longer link to: said, not hidden. */
  const missingNames = knowledge.loaded
    ? missingEntities(view, new Set(knowledge.facts.entities.keys()))
    : [];
  const toggleAuto = () =>
    auto.state.on
      ? auto.send({ type: "switchOff" })
      : auto.send({
          type: "switchOn",
          scopes,
          count: documentsUsed + tablesUsed,
          target: targetOf(shown, entries, Date.now()),
        });
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
          <div className="explorer__views" role="group" aria-label={t("explorer.viewSwitch")}>
            <button
              type="button"
              className="explorer__view"
              aria-pressed={mode === "files"}
              onClick={() => setMode("files")}
            >
              {t("explorer.viewFiles")}
            </button>
            <button
              type="button"
              className="explorer__view"
              aria-pressed={mode === "views"}
              onClick={() => {
                setMode("views");
                setPanelOpen(false);
                setOpenings((count) => count + 1);
              }}
            >
              {t("explorer.viewViews")}
            </button>
          </div>
        </div>
        {mode === "views" ? (
          <>
            <ExplorerViewsPanel
              saved={savedViews}
              current={view}
              facts={knowledge.facts}
              factsLoaded={knowledge.loaded}
              onOpen={openView}
              onSave={(name) =>
                onSavedViewsChange(
                  addView(savedViews, savedFromView(view, newViewId(savedViews, Date.now()), name)),
                )
              }
              onRename={(id, name) => onSavedViewsChange(renameView(savedViews, id, name))}
              onDelete={setDeletingView}
            />
            <ExplorerActivePills
              pills={pills}
              folders={folderOptions}
              foldersKnown={entries.length > 0}
              facts={knowledge.facts}
              factsLoaded={knowledge.loaded}
              onRemove={removeActivePill}
              onClear={clearFilters}
            />
          </>
        ) : (
          <>
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
                onClick={() => {
                  if (!panelOpen) {
                    setOpenings((count) => count + 1);
                  }
                  setPanelOpen((open) => !open);
                }}
              >
                {t("explorer.filters")}
                {pills.length === 0 ? null : (
                  <span
                    className="explorer__filters-badge"
                    aria-label={t("explorer.filtersActive", {
                      count: activeFilterCount(filter, typedFormats, switches),
                    })}
                  >
                    {activeFilterCount(filter, typedFormats, switches)}
                  </span>
                )}
              </button>
            </div>
            {idle ? null : (
              <ExplorerAutoSwitch
                on={auto.state.on}
                disabled={scopeLocked}
                onToggle={toggleAuto}
              />
            )}
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
                knowledge={
                  <ExplorerKnowledgeFilters
                    loaded={knowledge.loaded}
                    types={typeChoices(entries, filter)}
                    chosenTypes={choices.types}
                    years={yearChoices(entries, filter)}
                    chosenYears={choices.years}
                    people={entityOptions(entries, filter, "person", knowledge.facts)}
                    chosenPeople={choices.people}
                    organisations={entityOptions(entries, filter, "organisation", knowledge.facts)}
                    chosenOrganisations={choices.organisations}
                    disabled={idle}
                    onToggle={toggleFilter}
                    onRole={chooseRole}
                  />
                }
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
            {idle || notesKey === null || notesCount === 0 ? null : (
              <div className="explorer__notes">
                <button
                  type="button"
                  className="explorer__notes-chip"
                  aria-pressed={choices.folders.has(notesKey)}
                  title={t("explorer.notesHint")}
                  onClick={() => toggleFilter("folder", notesKey)}
                >
                  {t("explorer.notesChip")}
                  <small className="explorer__segment-count">{notesCount}</small>
                </button>
              </div>
            )}
            <ExplorerActivePills
              pills={pills}
              folders={folderOptions}
              foldersKnown={entries.length > 0}
              facts={knowledge.facts}
              factsLoaded={knowledge.loaded}
              onRemove={removeActivePill}
              onClear={clearFilters}
            />
            {missingNames.length > 0 ? (
              <p className="explorer__status" role="status">
                {missingNames.length === 1
                  ? t("explorer.viewMissingOne")
                  : t("explorer.viewMissingMany", { count: missingNames.length })}
              </p>
            ) : null}
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
          </>
        )}
      </header>

      {mode === "views" ? null : box === "firstStart" ? (
        <ExplorerFirstStart
          busy={folders.busy}
          canCreate={suggestedWorkFolder !== null || suggestedDataFolder !== null}
          onCreateBoth={() => void folders.createBoth()}
        />
      ) : null}

      {mode === "views" ? null : workFolder === null ? (
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
          needsAnalyse={documentsNeedsAnalyse}
          onOpenReport={onOpenReport}
          filtering={isFiltering(filter)}
          reasonOf={reasonOf}
        />
      )}
      {mode === "views" ? null : dataFolder === null ? (
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
          needsAnalyse={tablesNeedsAnalyse}
          onOpenReport={onOpenReport}
          filtering={isFiltering(filter)}
          reasonOf={reasonOf}
        />
      )}

      {error === null ? null : <ErrorBanner error={error} />}
      {folderError == null ? null : <ErrorBanner error={folderError} />}
      {indexing.error === null ? null : <ErrorBanner error={indexing.error} />}
      {dataIndexing.error === null ? null : <ErrorBanner error={dataIndexing.error} />}

      {idle ? null : (
        <ExplorerBottomBar
          documentsUsed={documentsUsed}
          tablesUsed={tablesUsed}
          toTick={showsTickLink(auto.state, toTick.length) ? toTick.length : 0}
          onTick={() => {
            auto.send({ type: "manualChange" });
            const next = addEntriesToScopes(scopes, toTick, entries, Date.now());
            onScopeChange(next.documents);
            onDataScopeChange(next.data);
          }}
          canClear={selectionOf(scope) !== "none" || selectionOf(dataScope) !== "none"}
          onClear={() => {
            auto.send({ type: "manualChange" });
            const next = clearedScopes(Date.now());
            onScopeChange(next.documents);
            onDataScopeChange(next.data);
          }}
          locked={scopeLocked}
          notes={selectionNotes(selectionOf(scope), selectionOf(dataScope))}
          unsupported={merged.unsupported}
          auto={{
            on: auto.state.on,
            unreachable: unreachableCount(shown),
            restore: restoreCount(auto.state),
            onRestore: () => auto.send({ type: "restore" }),
          }}
        />
      )}
      {deletingView === null ? null : (
        <ConfirmDialog
          title={t("explorer.viewDeleteTitle")}
          body={t("explorer.viewDeleteBody", {
            name: savedViews.find((saved) => saved.id === deletingView)?.name ?? "",
          })}
          confirmLabel={t("explorer.viewDelete")}
          destructive
          busy={false}
          onConfirm={() => {
            onSavedViewsChange(deleteView(savedViews, deletingView));
            setDeletingView(null);
          }}
          onCancel={() => setDeletingView(null)}
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
