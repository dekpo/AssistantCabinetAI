import { useCallback, useEffect, useMemo, useState } from "react";
import { AppLayout } from "./components/AppLayout";
import { ChatPanel } from "./components/ChatPanel";
import { DataFolderCard } from "./components/DataFolderCard";
import { ErrorBanner } from "./components/ErrorBanner";
import { isSelectable } from "./components/FolderCardParts";
import { KnowledgeDialog } from "./components/KnowledgeDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import { TitleBar } from "./components/TitleBar";
import { OutsideChangeNotice } from "./components/viewer/OutsideChangeNotice";
import { WorkFolderCard } from "./components/WorkFolderCard";
import { I18nProvider, useTranslation } from "./i18n/I18nProvider";
import {
  addScopeFiles,
  combineScopes,
  noDocumentsScope,
} from "./lib/analysisScope";
import {
  dataFolderInventory,
  type FileRecord,
  indexDataFolder,
  workFolderInventory,
  type AnalysisScope,
  type OutsideSelection,
} from "./lib/ipc";
import { NO_LAYOUTS } from "./lib/panelLayout";
import { applyTheme } from "./lib/theme";
import type { OpenWorkspace, WorkspaceView } from "./lib/workspaceView";
import { useAppSettings } from "./state/useAppSettings";
import { useOutsideChangeGuard } from "./state/useOutsideChangeGuard";
import { useIndexing } from "./state/useIndexing";
import { useServerHealth } from "./state/useServerHealth";

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

export default function App() {
  const { snapshot, locale, loadError, saveError, reload, update, saveLayout, reset } =
    useAppSettings();
  const { connection, health, error: healthError, refresh } = useServerHealth(
    snapshot?.settings.serverUrl,
  );
  /* Owned here rather than inside the folder card, because two places start the same pass: the
     card's own button, and the answer that had to say the documents have not been read yet. */
  const indexing = useIndexing();
  /* The Data Folder's own pass: two folders, two passes that never wait on each other. */
  const dataIndexing = useIndexing(indexDataFolder);
  const [settingsOpen, setSettingsOpen] = useState(false);
  /* The knowledge management dialog: opened from the settings or from an analysis summary, drawn
     over whichever is on screen. Never reachable from the conversation. */
  const [knowledgeOpen, setKnowledgeOpen] = useState(false);
  /* The documents the conversation is about. Held here because it is chosen in the folder card and
     used by the chat: nothing persists a session yet, so nothing persists this. It starts with no
     document: the selection is built only from what she ticks (`docs/SELECTION-AND-MEMORY.md`). */
  const [scope, setScope] = useState<AnalysisScope>(() => noDocumentsScope(Date.now()));
  /* The workbooks, chosen the same way in the Data Folder card and starting from none too. */
  const [dataScope, setDataScope] = useState<AnalysisScope>(() => noDocumentsScope(Date.now()));
  const [chatBusy, setChatBusy] = useState(false);
  /* What the centre zone of the window shows. The zone is open exactly while this is not null, so
     anything in the application can open it with `openWorkspace` and close it with
     `closeWorkspace` (`docs/CLIENT.md`, "Layout"). Nothing is open at launch. */
  const [workspace, setWorkspace] = useState<WorkspaceView | null>(null);
  const openWorkspace = useCallback<OpenWorkspace>((view) => setWorkspace(view), []);
  const closeWorkspace = useCallback(() => setWorkspace(null), []);
  /* The one modal of the viewer: the open file was moved, renamed, deleted or modified outside the
     software. The centre closes with it, and the file can only be shown again after an Analyse. */
  const [outsideNotice, setOutsideNotice] = useState(false);
  const fileLeft = useCallback(() => {
    setWorkspace(null);
    setOutsideNotice(true);
  }, []);
  useOutsideChangeGuard(workspace, fileLeft);
  /* "Open" on a row of either list: the centre shows that file, replacing what it showed. Opening
     changes no selection and starts no analysis. */
  const openDocument = useCallback(
    (record: FileRecord) => openWorkspace({ kind: "file", folder: "documents", record }),
    [openWorkspace],
  );
  const openTable = useCallback(
    (record: FileRecord) => openWorkspace({ kind: "file", folder: "data", record }),
    [openWorkspace],
  );
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
  /* Files chosen in one folder mean nothing in another: their paths would resolve to nothing and
     every question would be refused. A different folder starts from no document again. Analyse
     leaves the selection alone: each ticked file is pinned to its content and survives a pass. */
  useEffect(() => setScope(noDocumentsScope(Date.now())), [workFolder]);
  const dataFolder = snapshot?.settings.dataFolder ?? null;
  /* The same rule for the Data Folder: another folder, no workbook chosen. */
  useEffect(() => setDataScope(noDocumentsScope(Date.now())), [dataFolder]);
  /* Both lists, joined only when a question is sent (`docs/SELECTION-AND-MEMORY.md`). */
  const chatScope = useMemo(() => combineScopes(scope, dataScope), [scope, dataScope]);
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
        if (outside.documents.length > 0) {
          const analysed = (await workFolderInventory()).files.filter(isSelectable);
          const wanted = analysed.filter((file) => outside.documents.includes(file.relativePath));
          setScope((current) => addScopeFiles(current, wanted, analysed, now));
        }
        if (outside.workbooks.length > 0) {
          const analysed = (await dataFolderInventory()).files.filter(isSelectable);
          const wanted = analysed.filter((file) => outside.workbooks.includes(file.relativePath));
          setDataScope((current) => addScopeFiles(current, wanted, analysed, now));
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
      <div className="app">
        <TitleBar
          connection={connection}
          onRefresh={refresh}
          onOpenSettings={() => setSettingsOpen(true)}
          workspaceOpen={workspace !== null}
          onToggleWorkspace={() =>
            workspace === null ? openWorkspace({ kind: "welcome" }) : closeWorkspace()
          }
        />
        <main className="app__main">
          <AppLayout
            workspace={workspace}
            layouts={snapshot.settings.layout}
            onLayoutsChange={(change) => void saveLayout(change)}
            onWorkspaceCollapsed={closeWorkspace}
            onCloseWorkspace={closeWorkspace}
            onFileMissing={fileLeft}
            restoreNonce={layoutRestores}
            resources={
              <aside className="app__side">
                <WorkFolderCard
                  workFolder={snapshot.settings.workFolder}
                  suggestedWorkFolder={snapshot.suggestedWorkFolder}
                  onChosen={(path) => void update({ workFolder: path })}
                  indexing={indexing}
                  detail="collapsible"
                  scope={scope}
                  onScopeChange={setScope}
                  scopeLocked={chatBusy}
                  onOpenKnowledge={() => setKnowledgeOpen(true)}
                  onOpenFile={openDocument}
                />
                <DataFolderCard
                  dataFolder={snapshot.settings.dataFolder}
                  suggestedDataFolder={snapshot.suggestedDataFolder}
                  onChosen={(path) => void update({ dataFolder: path })}
                  indexing={dataIndexing}
                  detail="collapsible"
                  scope={dataScope}
                  onScopeChange={setDataScope}
                  scopeLocked={chatBusy}
                  onOpenKnowledge={() => setKnowledgeOpen(true)}
                  onOpenFile={openTable}
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
            onReset={async () => {
              await reset();
              setLayoutRestores((count) => count + 1);
            }}
            onRestoreLayout={() => {
              setLayoutRestores((count) => count + 1);
              void update({ layout: NO_LAYOUTS });
            }}
            onClose={() => setSettingsOpen(false)}
            onOpenKnowledge={() => setKnowledgeOpen(true)}
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
      </div>
    </I18nProvider>
  );
}
