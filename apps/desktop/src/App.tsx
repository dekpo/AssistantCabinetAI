import { useEffect, useMemo, useState } from "react";
import { ChatPanel } from "./components/ChatPanel";
import { DataFolderCard } from "./components/DataFolderCard";
import { ErrorBanner } from "./components/ErrorBanner";
import { SettingsDialog } from "./components/SettingsDialog";
import { TitleBar } from "./components/TitleBar";
import { WorkFolderCard } from "./components/WorkFolderCard";
import { I18nProvider, useTranslation } from "./i18n/I18nProvider";
import { combineScopes, noDocumentsScope } from "./lib/analysisScope";
import { indexDataFolder, type AnalysisScope } from "./lib/ipc";
import { applyTheme } from "./lib/theme";
import { useAppSettings } from "./state/useAppSettings";
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
  const { snapshot, locale, loadError, saveError, reload, update, reset } = useAppSettings();
  const { connection, health, error: healthError, refresh } = useServerHealth(
    snapshot?.settings.serverUrl,
  );
  /* Owned here rather than inside the folder card, because two places start the same pass: the
     card's own button, and the answer that had to say the documents have not been read yet. */
  const indexing = useIndexing();
  /* The Data Folder's own pass: two folders, two passes that never wait on each other. */
  const dataIndexing = useIndexing(indexDataFolder);
  const [settingsOpen, setSettingsOpen] = useState(false);
  /* The documents the conversation is about. Held here because it is chosen in the folder card and
     used by the chat: nothing persists a session yet, so nothing persists this. It starts with no
     document: the selection is built only from what she ticks (`docs/SELECTION-AND-MEMORY.md`). */
  const [scope, setScope] = useState<AnalysisScope>(() => noDocumentsScope(Date.now()));
  /* The workbooks, chosen the same way in the Data Folder card and starting from none too. */
  const [dataScope, setDataScope] = useState<AnalysisScope>(() => noDocumentsScope(Date.now()));
  const [chatBusy, setChatBusy] = useState(false);
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
        />
        <main className="app__main">
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
          <ChatPanel
            onFailure={refresh}
            hasWorkFolder={snapshot.settings.workFolder !== null}
            scope={chatScope}
            onBusyChange={setChatBusy}
            modelAlias={snapshot.settings.modelAlias}
            aliases={health?.aliases ?? []}
            indexing={indexing}
            onModelAliasChange={(alias) => void update({ modelAlias: alias })}
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
            onReset={reset}
            onClose={() => setSettingsOpen(false)}
          />
        ) : null}
      </div>
    </I18nProvider>
  );
}
