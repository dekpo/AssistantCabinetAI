import { useTranslation } from "../i18n/I18nProvider";
import type { ConnectionState } from "../state/useServerHealth";
import { ServerStatus } from "./ServerStatus";

export function TitleBar({
  connection,
  onRefresh,
  onOpenSettings,
}: {
  connection: ConnectionState;
  onRefresh: () => void;
  onOpenSettings: () => void;
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
        <button type="button" className="button" onClick={onOpenSettings}>
          {t("actions.openSettings")}
        </button>
      </div>
    </header>
  );
}
