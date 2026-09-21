import { useTranslation } from "../i18n/I18nProvider";
import type { ConnectionState } from "../state/useServerHealth";

export function ServerStatus({
  connection,
  onRefresh,
}: {
  connection: ConnectionState;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      className="status"
      onClick={onRefresh}
      title={t("status.recheck")}
      aria-label={t("status.recheck")}
    >
      <span className={`status__dot status__dot--${connection}`} aria-hidden="true" />
      <span className="status__value">{t(`status.${connection}`)}</span>
    </button>
  );
}
