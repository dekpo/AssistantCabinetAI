import { useTranslation } from "../i18n/I18nProvider";

/**
 * « Select. auto »: the optional switch that makes the selection follow the filters. Off by default and
 * off at every launch. It sits in the upper part of the box with its hint line, and is locked while a
 * question is being answered, like the selection (`lib/autoSelect.ts`).
 */
export function ExplorerAutoSwitch({
  on,
  disabled,
  onToggle,
}: {
  on: boolean;
  /** Nothing to select yet, or a question is being answered. */
  disabled: boolean;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="explorer-auto">
      <button
        type="button"
        role="switch"
        aria-checked={on}
        className="explorer-auto__switch"
        disabled={disabled}
        onClick={onToggle}
      >
        <span className="explorer-auto__track" aria-hidden="true">
          <span className="explorer-auto__thumb" />
        </span>
        <span className="explorer-auto__label">{t("explorer.autoLabel")}</span>
      </button>
      <small className="explorer-auto__hint">{t("explorer.autoHint")}</small>
    </div>
  );
}
