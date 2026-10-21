import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { AppLayout } from "./components/AppLayout";
import { ChatPanel } from "./components/ChatPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import { FileExplorer } from "./components/FileExplorer";
import { KnowledgeDialog } from "./components/KnowledgeDialog";
import type { NotesActions, NotesBundle } from "./components/notes/NotesPanel";
import type { NotesDictation } from "./components/notes/LiveDictation";
import {
  AnalysisChoiceDialog,
  RecordingDialog,
  UnsavedNoteDialog,
} from "./components/notes/NotesDialogs";
import { SettingsDialog } from "./components/SettingsDialog";
import { TitleBar } from "./components/TitleBar";
import { OutsideChangeNotice } from "./components/viewer/OutsideChangeNotice";
import type { OverviewActions } from "./components/OverviewPage";
import type { Centre, TabActions } from "./components/WorkspacePanel";
import { I18nProvider, useTranslation } from "./i18n/I18nProvider";
import { effectiveSection, shownCentre, type CentreSection } from "./lib/centreSection";
import { normaliseError, type AppError } from "./lib/errors";
import {
  addScopeFiles,
  combineScopes,
  noDocumentsScope,
} from "./lib/analysisScope";
import type { ExplorerEntry } from "./lib/explorerModel";
import { analysedByFamily } from "./lib/explorerSelection";
import { checkView, overviewInput, overviewModel, type ExplorerFacts } from "./lib/overviewModel";
import {
  familiesToUnfold,
  foldsToRestore,
  NO_VIEW,
  type ExplorerView,
} from "./lib/explorerView";
import type { Family } from "./lib/explorerModel";
import {
  dataFolderInventory,
  type FileRecord,
  indexDataFolder,
  revealDataFile,
  revealWorkFile,
  workFolderInventory,
  type AnalysisScope,
  type OutsideSelection,
} from "./lib/ipc";
import {
  activeDraft,
  guardExit,
  hasUnsaved,
  unsavedDrafts,
  type NotesExit,
} from "./lib/notesDrafts";
import { createNotesSession } from "./lib/notesSession";
import { NO_LAYOUTS } from "./lib/panelLayout";
import { applyTheme } from "./lib/theme";
import { createViewStateStore, type ViewStateStore } from "./lib/viewState";
import { closeWindow, guardWindowClose } from "./lib/windowClose";
import {
  activateTab,
  closeTab,
  closeTabs,
  idsOfFolder,
  openTab,
  tabsOf,
  viewOf,
  type TabsState,
} from "./lib/viewerTabs";
import type { ViewerFolder, WorkspaceView } from "./lib/workspaceView";
import { useAppSettings } from "./state/useAppSettings";
import { useOutsideChangeGuard } from "./state/useOutsideChangeGuard";
import { useIndexing } from "./state/useIndexing";
import { useServerHealth } from "./state/useServerHealth";
import { notesPreview, notesSave } from "./speech/ipc";
import { ReadAloudProvider } from "./speech/output/ReadAloudContext";
import { useLongDictation } from "./speech/useLongDictation";
import { useRepairSuggestions } from "./speech/useRepairSuggestions";
import { VoiceSessionProvider } from "./speech/VoiceSessionContext";

function StartupScreen({ onRetry, failed }: { onRetry: () => void; failed: boolean }) {
  const { t } = useTranslation();

  return (
    <div className="startup">
      {failed ? (
        <ErrorBanner error={{ code: "settings_read_failed", data: {} }} onRetry={onRetry} />
      ) : (
        <p className="startup__message">{t("app.loading")}</p>
      )}
    </div>
  );
}

function AppContent() {
  const {
    snapshot,
    locale,
    loadError,
    saveError,
    reload,
    update,
    saveLayout,
    saveExplorer,
    saveSpeech,
    reset,
  } = useAppSettings();
  const { connection, health, error: healthError, refresh } = useServerHealth(
    snapshot?.settings.serverUrl,
  );
  /* Owned here rather than inside the explorer, because two places start the same pass: the
     explorer's own button, and the answer that had to say the documents have not been read yet. */
  const indexing = useIndexing();
  /* The Data Folder's own pass: two folders, two passes that never wait on each other. */
  const dataIndexing = useIndexing(indexDataFolder);
  const [settingsOpen, setSettingsOpen] = useState(false);
  /* Counted up when an analysis is cleared from the Settings panel: the sidebar's box, which holds its
     own inventories, reads its folders again through its refresh token. */
  const [settingsClears, setSettingsClears] = useState(0);
  /* The knowledge management dialog: opened from the settings or from an analysis summary, drawn
     over whichever is on screen. Never reachable from the conversation. */
  const [knowledgeOpen, setKnowledgeOpen] = useState(false);
  /* The documents the conversation is about. Held here because it is chosen in the explorer and
     used by the chat: nothing persists a session yet, so nothing persists this. It starts with no
     document: the selection is built only from what she ticks (`docs/SELECTION-AND-MEMORY.md`). */
  const [scope, setScope] = useState<AnalysisScope>(() => noDocumentsScope(Date.now()));
  /* The workbooks, chosen the same way in the explorer and starting from none too. */
  const [dataScope, setDataScope] = useState<AnalysisScope>(() => noDocumentsScope(Date.now()));
  const [chatBusy, setChatBusy] = useState(false);
  /* What the centre zone of the window holds: the open tabs, or the welcome view. Nothing is open at
     launch. The zone is on screen while this is not null and not hidden: the title-bar button and a
     drag of the separator only hide it, so the tabs stay open (each keeps its view and comes back
     as it was), and anything that opens a file shows it again (`docs/CLIENT.md`, "Layout"). */
  const [workspace, setWorkspace] = useState<WorkspaceView | null>(null);
  const [workspaceHidden, setWorkspaceHidden] = useState(false);
  const hideWorkspace = useCallback(() => setWorkspaceHidden(true), []);
  /* The Notes section of the centre. The notes live in a session held here, in memory only, so a
     switch of section or a hidden centre never loses them; `section` is which of the two sections
     is on screen. The pass the Save button starts is this App's own Analyse (`indexing.run()`). */
  const indexingRef = useRef(indexing);
  indexingRef.current = indexing;
  const [notesSession] = useState(() =>
    createNotesSession({
      preview: notesPreview,
      save: notesSave,
      inventory: async () => (await workFolderInventory()).files,
      analyse: () => indexingRef.current.run(),
    }),
  );
  const notes = useSyncExternalStore(notesSession.subscribe, notesSession.getSnapshot);
  /* Files at launch, where the zone is closed; the title-bar button, « Voir le rapport » and the
     switch bring the overview, opening a file brings Files. */
  const [section, setSection] = useState<CentreSection>("files");
  const notesEnabled = snapshot?.settings.speech.notesEnabled ?? false;
  const shownSection = effectiveSection(section, notesEnabled);
  /* The voice of the notes: the microphone opens only when the pill is pressed. `undefined` while
     the voice is off, so no pill is drawn. A recording lives here, not in the panel, so that
     a section switch never touches it. */
  const longDictation = useLongDictation({
    speech: snapshot?.settings.speech ?? null,
    locale,
    session: notesSession,
    notesEnabled,
    saving: notes.phase !== "idle",
  });
  const dictationActive = longDictation?.active ?? false;
  const dictationActiveRef = useRef(false);
  dictationActiveRef.current = dictationActive;
  const cancelDictationRef = useRef<() => void>(() => undefined);
  cancelDictationRef.current = longDictation?.cancel ?? (() => undefined);
  /* What `AppLayout` is given: the zone stays open while Notes is shown or holds a note. */
  const shownWorkspace = shownCentre({
    workspace,
    hidden: workspaceHidden,
    notesEnabled,
    section,
    notesHold: hasUnsaved(notes.notes) || dictationActive,
  });
  /* What would lose a note asks first (`guardExit`); the question waits for the user. `proceed` is
     the exit itself, `abandon` what happens when the user cancels. */
  const [leaving, setLeaving] = useState<{
    exit: NotesExit;
    proceed: () => void;
    abandon?: () => void;
  } | null>(null);
  const guardLeaving = useCallback(
    (exit: NotesExit, proceed: () => void, abandon?: () => void) => {
      /* Every way out except the two that keep the Notes section mounted ends a recording. */
      const keeps = exit.kind === "switch-section" || exit.kind === "open-file";
      const leave = () => {
        if (!keeps) {
          cancelDictationRef.current();
        }
        proceed();
      };
      const recording = dictationActiveRef.current;
      if (guardExit(notesSession.getSnapshot().notes, exit, { recording }) === "allow") {
        leave();
      } else {
        setLeaving({ exit, proceed: leave, abandon });
      }
    },
    [notesSession],
  );
  const showNotes = useCallback(() => {
    setSection("notes");
    setWorkspaceHidden(false);
  }, []);
  /* Opens the zone on its overview: the way « Voir le rapport » and the title-bar button, with no
     file open, lead there. Nothing opens the zone by itself: a finished analysis only changes what
     the overview says. */
  const showOverview = useCallback(() => {
    setSection("overview");
    setWorkspaceHidden(false);
  }, []);
  /* The cross of the window: a note that is not saved keeps it open until the user has answered. */
  useEffect(
    () =>
      guardWindowClose(() => {
        if (!notesSession.hasUnsaved() && !dictationActiveRef.current) {
          return false;
        }
        setLeaving({
          exit: { kind: "close-window" },
          proceed: () => {
            cancelDictationRef.current();
            void closeWindow();
          },
        });
        return true;
      }),
    [notesSession],
  );
  /* The one modal of the viewer: the open file was moved, renamed, deleted or modified outside the
     software. The centre closes with it, and the file can only be shown again after an Analyse. */
  const [outsideNotice, setOutsideNotice] = useState(false);
  /* What each open tab remembered of its view (scroll, zoom, sheet, slide, Markdown mode). Memory
     only: dropped with the tab, never stored. */
  const [viewStates] = useState<ViewStateStore>(createViewStateStore);
  /* Every change of the open tabs goes through here; a tab list that becomes empty closes the
     centre, and a change that changes nothing leaves the view (the welcome view too) as it was. */
  const changeTabs = useCallback(
    (change: (state: TabsState) => TabsState) =>
      setWorkspace((current) => {
        const before = tabsOf(current);
        const after = change(before);
        return after === before ? current : viewOf(after);
      }),
    [],
  );
  /* The one way to open a file, so that a cap on the number of tabs is a change in `openTab` only.
     "Open" on a row of either list: a file already open is shown again, never duplicated. Opening
     changes no selection and starts no analysis. */
  const openFile = useCallback(
    (folder: ViewerFolder, record: FileRecord) => {
      changeTabs((state) => openTab(state, folder, record));
      setWorkspaceHidden(false);
      /* A file opened from a list is shown: the Files section, with Notes left alive behind it. */
      setSection("files");
    },
    [changeTabs],
  );
  const openDocument = useCallback(
    (record: FileRecord) => openFile("documents", record),
    [openFile],
  );
  const openTable = useCallback((record: FileRecord) => openFile("data", record), [openFile]);
  /* Tabs whose file is no longer as it was opened: closed together, with one modal for all. */
  const tabsLeft = useCallback(
    (ids: string[]) => {
      changeTabs((state) => closeTabs(state, ids));
      setOutsideNotice(true);
    },
    [changeTabs],
  );
  const tabActions = useMemo<TabActions>(
    () => ({
      activate: (id) => changeTabs((state) => activateTab(state, id)),
      close: (id) => changeTabs((state) => closeTab(state, id)),
      missing: (id) => tabsLeft([id]),
    }),
    [changeTabs, tabsLeft],
  );
  const openTabs = useMemo(() => tabsOf(workspace).tabs, [workspace]);
  useOutsideChangeGuard(openTabs, tabsLeft);
  /* A closed tab forgets its view, whoever closed it (the user, the guard, a folder change). */
  useEffect(() => viewStates.retain(openTabs.map((tab) => tab.id)), [openTabs, viewStates]);
  /* Counted up whenever the default widths of the zones must be put back on screen at once. */
  const [layoutRestores, setLayoutRestores] = useState(0);
  /* Suggestions that name somebody are hidden by default: a screen can be seen by whoever waits in
     the room. The user's own click shows them for this session only; nothing is stored, and the
     setting itself is not changed. */
  const [revealNames, setRevealNames] = useState(false);
  /* A suggestion chosen under an analysis summary asks its question in the conversation. */
  const [requestedQuestion, setRequestedQuestion] = useState<{
    text: string;
    nonce: number;
  } | null>(null);
  const workFolder = snapshot?.settings.workFolder ?? null;
  const dataFolder = snapshot?.settings.dataFolder ?? null;
  const notesActions = useMemo<NotesActions>(
    () => ({
      select: notesSession.select,
      add: notesSession.add,
      edit: notesSession.edit,
      discard: (id) =>
        guardLeaving({ kind: "discard-draft", id }, () => notesSession.remove(id)),
      review: (scope) => void notesSession.review(scope),
      confirm: () => void notesSession.confirm(),
      cancelPlan: notesSession.cancelPlan,
      dismissReport: notesSession.dismissReport,
      reveal: revealWorkFile,
    }),
    [notesSession, guardLeaving],
  );
  /* Another Documents folder starts from no note: the notes written for this one would otherwise
     land in the next. The first choice of a folder loses nothing. The one door to a change of folder
     is the settings panel until the explorer's own menu (`docs/CLIENT.md`, "The left panel"). */
  const chooseWorkFolder = useCallback(
    (path: string) =>
      workFolder === null || path === workFolder
        ? void update({ workFolder: path })
        : guardLeaving({ kind: "change-folder" }, () => {
            notesSession.discardAll();
            void update({ workFolder: path });
          }),
    [workFolder, update, guardLeaving, notesSession],
  );
  /* Files | Notes. A centre with no tab open would close when Files is chosen, so Files shows its
     welcome view then. Switching is never a way to lose a note. */
  const chooseSection = useCallback(
    (next: CentreSection) => {
      setSection(next);
      if (next === "files" && workspace === null) {
        setWorkspace({ kind: "welcome" });
      }
    },
    [workspace],
  );
  /* Files chosen in one folder mean nothing in another: their paths would resolve to nothing and
     every question would be refused. A different folder starts from no document again. Analyse
     leaves the selection alone: each ticked file is pinned to its content and survives a pass. */
  useEffect(() => setScope(noDocumentsScope(Date.now())), [workFolder]);
  /* The same rule for the Data Folder: another folder, no workbook chosen. */
  useEffect(() => setDataScope(noDocumentsScope(Date.now())), [dataFolder]);
  /* The tabs of a folder that was replaced would show another folder's file under the same path:
     they close, and their view with them. */
  useEffect(
    () => changeTabs((state) => closeTabs(state, idsOfFolder(state, "documents"))),
    [workFolder, changeTabs],
  );
  useEffect(
    () => changeTabs((state) => closeTabs(state, idsOfFolder(state, "data"))),
    [dataFolder, changeTabs],
  );
  /* Both lists, joined only when a question is sent (`docs/SELECTION-AND-MEMORY.md`). */
  const chatScope = useMemo(() => combineScopes(scope, dataScope), [scope, dataScope]);
  /* The name suggestions for the words just dictated into the active note: the files of the
     selection, names hidden until the user asks, nothing replaced without a click. */
  const activeNote = activeDraft(notes.notes);
  const setActiveNoteText = useCallback(
    (text: string) => notesSession.edit(activeNote.id, { text }),
    [notesSession, activeNote.id],
  );
  const repair = useRepairSuggestions({
    source: longDictation?.heard ?? null,
    draft: activeNote.text,
    setDraft: setActiveNoteText,
    scope: chatScope,
    revealNames,
    onRevealNamesChange: setRevealNames,
    enabled: longDictation !== undefined,
  });
  const notesDictation = useMemo<NotesDictation | null>(
    () =>
      longDictation === undefined
        ? null
        : {
            phase: longDictation.phase,
            recording: longDictation.recording,
            disabled: longDictation.disabled,
            reasonKey: longDictation.reasonKey,
            level: longDictation.level,
            startedAt: longDictation.startedAt,
            waiting: longDictation.waiting,
            failed: longDictation.failed,
            error: longDictation.error,
            capReached: longDictation.capReached,
            onToggle: longDictation.toggle,
            onCancel: longDictation.cancel,
            onDismiss: longDictation.dismiss,
            repair,
          },
    [longDictation, repair],
  );
  const notesBundle = useMemo<NotesBundle>(
    () => ({
      snapshot: notes,
      actions: notesActions,
      hasFolder: workFolder !== null,
      dictation: notesDictation,
    }),
    [notes, notesActions, workFolder, notesDictation],
  );
  /* The explorer's filter, held here so that the overview's link can set what the panel sets, and
     the counter that asks the typed search to be emptied with it. Neither is the selection. */
  const [explorerView, setExplorerView] = useState<ExplorerView>(NO_VIEW);
  const [searchClears, setSearchClears] = useState(0);
  /* What the explorer read of the two folders, for the overview. */
  const [explorerFacts, setExplorerFacts] = useState<ExplorerFacts | null>(null);
  /* Read by the overview's link at the moment it is pressed, so its action stays one stable value. */
  /* The lists the overview's link opened: they close again when the filter it set is removed. */
  const [linkOpened, setLinkOpened] = useState<Family[]>([]);
  const explorerFactsRef = useRef(explorerFacts);
  explorerFactsRef.current = explorerFacts;
  const settingsRef = useRef(snapshot?.settings);
  settingsRef.current = snapshot?.settings;
  const overviewOnScreen = shownWorkspace !== null && shownSection === "overview";
  const overviewPage = useMemo(
    () =>
      overviewOnScreen
        ? overviewModel(
            overviewInput(
              explorerFacts,
              { documents: workFolder, tables: dataFolder },
              {
                summary: indexing.summary,
                durationMs: indexing.durationMs,
                finishedAt: indexing.finishedAt,
                running: indexing.running,
                error: indexing.error,
              },
              {
                summary: dataIndexing.summary,
                durationMs: dataIndexing.durationMs,
                finishedAt: dataIndexing.finishedAt,
                running: dataIndexing.running,
                error: dataIndexing.error,
              },
            ),
          )
        : null,
    [
      overviewOnScreen,
      explorerFacts,
      workFolder,
      dataFolder,
      indexing.summary,
      indexing.durationMs,
      indexing.finishedAt,
      indexing.running,
      indexing.error,
      dataIndexing.summary,
      dataIndexing.durationMs,
      dataIndexing.finishedAt,
      dataIndexing.running,
      dataIndexing.error,
    ],
  );
  /* Removing the filter the link set closes what the link opened, and only that. */
  useEffect(() => {
    const close = foldsToRestore(linkOpened, explorerView);
    if (close.length === 0) {
      return;
    }
    setLinkOpened([]);
    void saveExplorer((current) => ({
      ...current,
      documentsFolded: close.includes("documents") ? true : current.documentsFolded,
      tablesFolded: close.includes("tables") ? true : current.tablesFolded,
    }));
  }, [linkOpened, explorerView, saveExplorer]);
  const overviewActions = useMemo<OverviewActions>(
    () => ({
      onShowUnreadable: () => {
        setExplorerView(checkView());
        setSearchClears((count) => count + 1);
        /* A folded list that holds some of these files is opened: that is what the link was
           pressed for. Nothing else ever opens or closes a list by itself. */
        const open = familiesToUnfold(explorerFactsRef.current?.entries ?? [], "unreadable", {
          documents: settingsRef.current?.explorer.documentsFolded ?? false,
          tables: settingsRef.current?.explorer.tablesFolded ?? false,
        });
        if (open.length > 0) {
          setLinkOpened((current) => [...new Set([...current, ...open])]);
          void saveExplorer((current) => ({
            ...current,
            documentsFolded: open.includes("documents") ? false : current.documentsFolded,
            tablesFolded: open.includes("tables") ? false : current.tablesFolded,
          }));
        }
      },
      onOpenKnowledge: () => setKnowledgeOpen(true),
      onOpenFile: (entry: ExplorerEntry) =>
        entry.root === "documents" ? openDocument(entry.record) : openTable(entry.record),
      onRevealFile: (entry: ExplorerEntry, onFailure: (error: AppError) => void) =>
        void (entry.root === "documents" ? revealWorkFile(entry.path) : revealDataFile(entry.path)).catch(
          (raw: unknown) => onFailure(normaliseError(raw)),
        ),
    }),
    [saveExplorer, openDocument, openTable],
  );
  const centreProps: Centre = {
    section: shownSection,
    onSection: chooseSection,
    recording: longDictation?.recording ?? false,
    notes: notesEnabled ? notesBundle : undefined,
    overview: { page: overviewPage, actions: overviewActions },
  };
  /* "Add to my selection", under the name picker: the files of a name that is only in analysed
     files she did not tick. Her own click, and the same ticking as the listing's checkboxes
     (`addScopeFiles` shares its rule with `toggleScopeFile`). The ids come from a fresh read of each
     folder, because a ticked file is pinned to its content. A read that fails changes nothing: the
     offer stays on screen and she can press it again. Locked while an answer is being written, as
     the listings are. */
  const addOutsideToSelection = useCallback(
    async (outside: OutsideSelection) => {
      if (chatBusy) {
        return;
      }
      const now = Date.now();
      try {
        /* The names the base holds are in files by the key the index knows them by, which names the
           folder of a file that sits in the other one: the analysed files of each family come from
           both folders (`analysedByFamily`). */
        const [documentsRead, dataRead] = await Promise.all([
          workFolderInventory().catch(() => null),
          dataFolderInventory().catch(() => null),
        ]);
        const analysed = analysedByFamily(documentsRead?.files ?? [], dataRead?.files ?? []);
        if (outside.documents.length > 0) {
          const wanted = analysed.documents.filter((file) =>
            outside.documents.includes(file.relativePath),
          );
          setScope((current) => addScopeFiles(current, wanted, analysed.documents, now));
        }
        if (outside.workbooks.length > 0) {
          const wanted = analysed.data.filter((file) =>
            outside.workbooks.includes(file.relativePath),
          );
          setDataScope((current) => addScopeFiles(current, wanted, analysed.data, now));
        }
      } catch {
        // Nothing was changed; see above.
      }
    },
    [chatBusy],
  );
  /* The questions are looked for again whenever a pass finished (the base may have changed) or a
     setting that decides what is shown changed (the mode, whether names are hidden, the language),
     and never while a pass is writing. */
  const suggestionsRefresh = [
    indexing.finishedPasses,
    dataIndexing.finishedPasses,
    snapshot?.settings.knowledgeMode,
    snapshot?.settings.knowledgeHideNamesInSuggestions,
    snapshot?.settings.locale,
  ].join("-");
  const suggestionsEnabled = !indexing.running && !dataIndexing.running;
  const theme = snapshot?.settings.theme ?? "system";
  const localeIsStored = snapshot !== null && snapshot.settings.locale !== null;

  useEffect(() => applyTheme(theme), [theme]);

  useEffect(() => {
    // First launch: write down the language resolved from the system, so the gateway is told
    // which language to answer in rather than falling back to its own default.
    if (snapshot !== null && !localeIsStored) {
      void update({ locale });
    }
  }, [snapshot, localeIsStored, locale, update]);

  useEffect(() => {
    // A renamed or removed alias in MODEL_ALIASES (`docs/TROUBLESHOOTING.md`) must not brick a
    // machine that saved the old name: self-heal to whatever the gateway reports as current the
    // moment health confirms the saved choice no longer exists, rather than silently mismatching
    // in the Settings dropdown or failing every chat/index call until a human edits settings.json
    // by hand.
    if (snapshot === null || health === null) {
      return;
    }
    const patch: { modelAlias?: string; embeddingAlias?: string } = {};
    if (health.aliases.length > 0 && !health.aliases.includes(snapshot.settings.modelAlias)) {
      patch.modelAlias = health.defaultModelAlias;
    }
    if (snapshot.settings.embeddingAlias !== health.embeddingAlias) {
      patch.embeddingAlias = health.embeddingAlias;
    }
    if (Object.keys(patch).length > 0) {
      void update(patch);
    }
  }, [snapshot, health, update]);

  if (snapshot === null) {
    return (
      <I18nProvider locale={locale}>
        <StartupScreen onRetry={reload} failed={loadError !== null} />
      </I18nProvider>
    );
  }

  return (
    <I18nProvider locale={locale}>
      <ReadAloudProvider speech={snapshot.settings.speech}>
      <div className="app">
        <TitleBar
          connection={connection}
          onRefresh={refresh}
          onOpenSettings={() => setSettingsOpen(true)}
          workspaceOpen={shownWorkspace !== null}
          onToggleWorkspace={() => {
            if (shownWorkspace !== null) {
              guardLeaving({ kind: "hide-zone" }, hideWorkspace);
            } else if (workspace === null) {
              showOverview();
            } else {
              setWorkspaceHidden(false);
            }
          }}
          notes={
            notesEnabled
              ? {
                  open: shownWorkspace !== null && shownSection === "notes",
                  onToggle: () =>
                    shownWorkspace !== null && shownSection === "notes"
                      ? guardLeaving({ kind: "hide-zone" }, hideWorkspace)
                      : showNotes(),
                }
              : undefined
          }
        />
        <main className="app__main">
          <AppLayout
            workspace={shownWorkspace}
            layouts={snapshot.settings.layout}
            onLayoutsChange={(change) => void saveLayout(change)}
            onWorkspaceCollapsed={() => {
              /* A drag of the separator cannot be intercepted: the zone is already closed. A recording
                 must not go on out of sight with the microphone open, so it ends here; what was
                 already written stays in its note. */
              cancelDictationRef.current();
              hideWorkspace();
            }}
            viewStates={viewStates}
            tabActions={tabActions}
            centre={centreProps}
            restoreNonce={layoutRestores}
            resources={
              <aside className="app__side">
                <FileExplorer
                  workFolder={snapshot.settings.workFolder}
                  dataFolder={snapshot.settings.dataFolder}
                  suggestedWorkFolder={snapshot.suggestedWorkFolder}
                  suggestedDataFolder={snapshot.suggestedDataFolder}
                  onWorkFolderChosen={chooseWorkFolder}
                  onDataFolderChosen={(path) => void update({ dataFolder: path })}
                  indexing={indexing}
                  dataIndexing={dataIndexing}
                  scope={scope}
                  dataScope={dataScope}
                  onScopeChange={setScope}
                  onDataScopeChange={setDataScope}
                  scopeLocked={chatBusy}
                  density={snapshot.settings.explorer.density}
                  folded={{
                    documents: snapshot.settings.explorer.documentsFolded,
                    tables: snapshot.settings.explorer.tablesFolded,
                  }}
                  view={explorerView}
                  onViewChange={setExplorerView}
                  searchClears={searchClears}
                  onFacts={setExplorerFacts}
                  onOpenReport={showOverview}
                  savedViews={snapshot.settings.explorerViews}
                  onSavedViewsChange={(views) => void update({ explorerViews: views })}
                  notesFolder={snapshot.settings.speech.notesFolderName}
                  onFoldedChange={(family, isFolded) => {
                    /* A fold she makes herself is hers: the link's memory is dropped. */
                    setLinkOpened([]);
                    void saveExplorer((current) =>
                      family === "documents"
                        ? { ...current, documentsFolded: isFolded }
                        : { ...current, tablesFolded: isFolded },
                    );
                  }}
                  onOpenDocument={openDocument}
                  onOpenTable={openTable}
                  refreshToken={notes.writes + settingsClears}
                />
                {snapshot.warnings.map((code) => (
                  <ErrorBanner key={code} error={{ code, data: {} }} />
                ))}
                {healthError === null ? null : (
                  <div className="app__pinned">
                    <ErrorBanner error={healthError} onRetry={refresh} />
                  </div>
                )}
              </aside>
            }
            chat={
              <ChatPanel
                onFailure={refresh}
                hasWorkFolder={snapshot.settings.workFolder !== null}
                scope={chatScope}
                onBusyChange={setChatBusy}
                modelAlias={snapshot.settings.modelAlias}
                aliases={health?.aliases ?? []}
                indexing={indexing}
                onModelAliasChange={(alias) => void update({ modelAlias: alias })}
                showDiagnostics={snapshot.settings.showDiagnostics}
                copySources={snapshot.settings.copySources}
                onAddToSelection={(outside) => void addOutsideToSelection(outside)}
                revealNames={revealNames}
                onRevealNamesChange={setRevealNames}
                suggestionsRefresh={suggestionsRefresh}
                suggestionsEnabled={suggestionsEnabled}
                requestedQuestion={requestedQuestion}
                onRequestedQuestionHandled={() => setRequestedQuestion(null)}
                speech={snapshot.settings.speech}
              />
            }
          />
        </main>
        {settingsOpen ? (
          <SettingsDialog
            settings={snapshot.settings}
            settingsPath={snapshot.settingsPath}
            suggestedWorkFolder={snapshot.suggestedWorkFolder}
            suggestedDataFolder={snapshot.suggestedDataFolder}
            aliases={health?.aliases ?? []}
            saveError={saveError}
            indexing={indexing}
            dataIndexing={dataIndexing}
            onUpdate={(patch) => void update(patch)}
            onWorkFolderChosen={chooseWorkFolder}
            onSaveExplorer={(change) => void saveExplorer(change)}
            onSaveSpeech={(change) =>
              snapshot.settings.speech.notesEnabled && !change(snapshot.settings.speech).notesEnabled
                ? guardLeaving({ kind: "disable-notes" }, () => {
                    notesSession.discardAll();
                    void saveSpeech(change);
                  })
                : void saveSpeech(change)
            }
            onReset={() =>
              new Promise<void>((resolve, reject) =>
                guardLeaving(
                  { kind: "reset-settings" },
                  () => {
                    notesSession.discardAll();
                    reset()
                      .then(() => setLayoutRestores((count) => count + 1))
                      .then(resolve, reject);
                  },
                  resolve,
                ),
              )
            }
            onRestoreLayout={() => {
              setLayoutRestores((count) => count + 1);
              void update({ layout: NO_LAYOUTS });
            }}
            onClose={() => setSettingsOpen(false)}
            onOpenKnowledge={() => setKnowledgeOpen(true)}
            onOpenReport={() => {
              setSettingsOpen(false);
              showOverview();
            }}
            onFolderCleared={() => setSettingsClears((count) => count + 1)}
          />
        ) : null}
        {outsideNotice ? (
          <OutsideChangeNotice onAcknowledge={() => setOutsideNotice(false)} />
        ) : null}
        {knowledgeOpen ? (
          <KnowledgeDialog
            knowledgeMode={snapshot.settings.knowledgeMode}
            onClose={() => setKnowledgeOpen(false)}
          />
        ) : null}
        {notes.phase === "asking" ? (
          <AnalysisChoiceDialog
            onAnalyse={() => notesSession.answer(true)}
            onSkip={() => notesSession.answer(false)}
          />
        ) : null}
        {leaving === null ? null : leaving.exit.kind === "hide-zone" ||
          (unsavedDrafts(notes.notes).length === 0 && leaving.exit.kind !== "discard-draft") ? (
          <RecordingDialog
            onStop={() => {
              setLeaving(null);
              leaving.proceed();
            }}
            onCancel={() => {
              leaving.abandon?.();
              setLeaving(null);
            }}
          />
        ) : (
          <UnsavedNoteDialog
            count={leaving.exit.kind === "discard-draft" ? 1 : unsavedDrafts(notes.notes).length}
            recording={dictationActive}
            onCancel={() => {
              leaving.abandon?.();
              setLeaving(null);
            }}
            onDiscard={() => {
              setLeaving(null);
              leaving.proceed();
            }}
            onReview={() => {
              leaving.abandon?.();
              setLeaving(null);
              /* The exit is given up: the user is taken to the exact files that would be written,
                 and the settings, if they asked, are put out of the way. A recording in progress
                 ends first: the notes cannot change under the plan. */
              cancelDictationRef.current();
              setSettingsOpen(false);
              if (leaving.exit.kind === "discard-draft") {
                notesSession.select(leaving.exit.id);
                void notesSession.review("active");
              } else {
                void notesSession.review("all");
              }
              showNotes();
            }}
          />
        )}
      </div>
      </ReadAloudProvider>
    </I18nProvider>
  );
}

/** One microphone for the whole window: the voice session is created here, once, and renders nothing. */
export default function App() {
  return (
    <VoiceSessionProvider>
      <AppContent />
    </VoiceSessionProvider>
  );
}
