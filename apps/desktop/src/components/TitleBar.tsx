import { useTranslation } from "../i18n/I18nProvider";
import type { ConnectionState } from "../state/useServerHealth";
import { ServerStatus } from "./ServerStatus";
import { NotesGlyph } from "./NotesGlyph";
import { WorkspaceGlyph } from "./WorkspaceGlyph";

export function TitleBar({
  connection,
  onRefresh,
  onOpenSettings,
  workspaceOpen,
  onToggleWorkspace,
  notes,
}: {
  connection: ConnectionState;
  onRefresh: () => void;
  onOpenSettings: () => void;
  /** Whether the centre zone of the window is shown. */
  workspaceOpen: boolean;
  onToggleWorkspace: () => void;
  /** The Notes button, between the workspace toggle and Settings. Absent while the Notes switch is
   * off: the bar then draws exactly what it drew before. */
  notes?: { open: boolean; onToggle: () => void };
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
        {notes === undefined ? null : (
          <button
            type="button"
            className="button button--icon"
            aria-pressed={notes.open}
            aria-label={t("notes.toggle")}
            title={t("notes.toggle")}
            onClick={notes.onToggle}
          >
            <NotesGlyph />
          </button>
        )}
        <button type="button" className="button" onClick={onOpenSettings}>
          {t("actions.openSettings")}
        </button>
      </div>
    </header>
  );
}
