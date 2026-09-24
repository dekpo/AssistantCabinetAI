import { useEffect, useState } from "react";
import { ChatPanel } from "./components/ChatPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import { SettingsDialog } from "./components/SettingsDialog";
import { TitleBar } from "./components/TitleBar";
import { WorkFolderCard } from "./components/WorkFolderCard";
import { I18nProvider, useTranslation } from "./i18n/I18nProvider";
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
  const [settingsOpen, setSettingsOpen] = useState(false);
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
            />
            {snapshot.warnings.map((code) => (
              <ErrorBanner key={code} error={{ code, data: {} }} />
            ))}
            {healthError === null ? null : <ErrorBanner error={healthError} onRetry={refresh} />}
          </aside>
          <ChatPanel
            onFailure={refresh}
            hasWorkFolder={snapshot.settings.workFolder !== null}
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
            aliases={health?.aliases ?? []}
            saveError={saveError}
            indexing={indexing}
            onUpdate={(patch) => void update(patch)}
            onReset={reset}
            onClose={() => setSettingsOpen(false)}
          />
        ) : null}
      </div>
    </I18nProvider>
  );
}
