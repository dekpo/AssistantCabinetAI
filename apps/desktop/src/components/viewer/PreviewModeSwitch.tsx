import { useTranslation } from "../../i18n/I18nProvider";
import type { PreviewMode } from "../../lib/viewerFormats";

const LABEL_KEYS: Readonly<Record<PreviewMode, string>> = {
  raw: "viewer.modeRaw",
  preview: "viewer.modePreview",
};

/**
 * The Raw | Preview switch: two buttons, the pressed one carries the accent. Plain buttons with
 * `aria-pressed` keep it reachable and readable with the keyboard alone.
 */
export function PreviewModeSwitch({
  modes,
  mode,
  onChange,
}: {
  modes: readonly PreviewMode[];
  mode: PreviewMode;
  onChange: (mode: PreviewMode) => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="viewer__controls">
      <div className="viewer__modes" role="group" aria-label={t("viewer.modeLabel")}>
        {modes.map((entry) => (
          <button
            key={entry}
            type="button"
            className="button button--compact viewer__mode"
            aria-pressed={entry === mode}
            onClick={() => onChange(entry)}
          >
            {t(LABEL_KEYS[entry])}
          </button>
        ))}
      </div>
    </div>
  );
}
