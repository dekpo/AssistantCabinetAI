import { useEffect, useState } from "react";
import { ChatPanel } from "./components/ChatPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import { SettingsDialog } from "./components/SettingsDialog";
import { TitleBar } from "./components/TitleBar";
import { WorkFolderCard } from "./components/WorkFolderCard";
import { I18nProvider, useTranslation } from "./i18n/I18nProvider";
import { applyTheme } from "./lib/theme";
import { useAppSettings } from "./state/useAppSettings";
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
  const { snapshot, locale, loadError, saveError, reload, update } = useAppSettings();
  const { connection, health, error: healthError, refresh } = useServerHealth(
    snapshot?.settings.serverUrl,
  );
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
          />
        </main>
        {settingsOpen ? (
          <SettingsDialog
            settings={snapshot.settings}
            settingsPath={snapshot.settingsPath}
            suggestedWorkFolder={snapshot.suggestedWorkFolder}
            aliases={health?.aliases ?? []}
            saveError={saveError}
            onUpdate={(patch) => void update(patch)}
            onClose={() => setSettingsOpen(false)}
          />
        ) : null}
      </div>
    </I18nProvider>
  );
}
