import { useTranslation } from "../i18n/I18nProvider";
import { errorMessage, type AppError } from "../lib/errors";

export function ErrorBanner({ error, onRetry }: { error: AppError; onRetry?: () => void }) {
  const { t } = useTranslation();

  return (
    <div className="banner" role="alert">
      <p className="banner__title">{t("errorBanner.title")}</p>
      <p className="banner__message">{errorMessage(t, error)}</p>
      {onRetry === undefined ? null : (
        <button type="button" className="button button--quiet" onClick={onRetry}>
          {t("actions.retry")}
        </button>
      )}
    </div>
  );
}
