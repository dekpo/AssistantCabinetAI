import { useTranslation } from "../i18n/I18nProvider";
import { errorMessage, type AppError } from "../lib/errors";

/**
 * `onDismiss` is offered where a banner would otherwise sit in the way. A failure that reports a
 * state the software is still in - a work folder that no longer passes the rules - has no dismiss
 * on purpose: closing it would hide something that is still true. One that reports a moment that
 * has passed, like an answer that ran out of patience, does.
 */
export function ErrorBanner({
  error,
  onRetry,
  onDismiss,
}: {
  error: AppError;
  onRetry?: () => void;
  onDismiss?: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="banner" role="alert">
      <div className="banner__head">
        <p className="banner__title">{t("errorBanner.title")}</p>
        {onDismiss === undefined ? null : (
          <button
            type="button"
            className="banner__close"
            onClick={onDismiss}
            aria-label={t("actions.close")}
          >
            <CloseGlyph />
          </button>
        )}
      </div>
      <p className="banner__message">{errorMessage(t, error)}</p>
      {onRetry === undefined ? null : (
        <button type="button" className="button button--quiet" onClick={onRetry}>
          {t("actions.retry")}
        </button>
      )}
    </div>
  );
}

/** Drawn rather than typed, so the mark is the same in every language and at every font size. */
function CloseGlyph() {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 12 12"
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    >
      <path d="M2.5 2.5 L9.5 9.5 M9.5 2.5 L2.5 9.5" />
    </svg>
  );
}
