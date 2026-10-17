import { useTranslation } from "../i18n/I18nProvider";
import type { ConnectionState } from "../state/useServerHealth";
import { ServerStatus } from "./ServerStatus";
import { WorkspaceGlyph } from "./WorkspaceGlyph";

export function TitleBar({
  connection,
  onRefresh,
  onOpenSettings,
  workspaceOpen,
  onToggleWorkspace,
}: {
  connection: ConnectionState;
  onRefresh: () => void;
  onOpenSettings: () => void;
  /** Whether the centre zone of the window is shown. */
  workspaceOpen: boolean;
  onToggleWorkspace: () => void;
}) {
  const { t } = useTranslation();

  return (
    <header className="titlebar">
      <div>
        {/* The only product name on screen: no tool name, no logo, no address. */}
        <p className="titlebar__name">{t("app.name")}</p>
        <p className="titlebar__subtitle">{t("app.subtitle")}</p>
      </div>
      <div className="titlebar__actions">
        <ServerStatus connection={connection} onRefresh={onRefresh} />
        <button
          type="button"
          className="button button--icon"
          aria-pressed={workspaceOpen}
          aria-label={t("workspace.toggle")}
          title={t("workspace.toggle")}
          onClick={onToggleWorkspace}
        >
          <WorkspaceGlyph />
        </button>
        <button type="button" className="button" onClick={onOpenSettings}>
          {t("actions.openSettings")}
        </button>
      </div>
    </header>
  );
}
