import { useTranslation } from "../../i18n/I18nProvider";
import type { PreviewKind, PreviewMode } from "../../lib/viewerFormats";

const RAW_KEY = "viewer.modeRaw";

/**
 * The second mode is named for what it shows, so a reading of the text can never be taken for the
 * original: a rendering of Markdown is a "preview", the text the assistant reads is said to be that.
 */
const SECOND_MODE_KEYS: Readonly<Record<PreviewKind, string>> = {
  markdown: "viewer.modePreview",
  readable: "viewer.modeReadable",
};

/**
 * The Raw | second-mode switch: two buttons, the pressed one carries the accent. Plain buttons with
 * `aria-pressed` keep it reachable and readable with the keyboard alone.
 */
export function PreviewModeSwitch({
  modes,
  mode,
  kind,
  onChange,
}: {
  modes: readonly PreviewMode[];
  mode: PreviewMode;
  kind: PreviewKind;
  onChange: (mode: PreviewMode) => void;
}) {
  const { t } = useTranslation();
  const labelKey = (entry: PreviewMode) => (entry === "raw" ? RAW_KEY : SECOND_MODE_KEYS[kind]);

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
            {t(labelKey(entry))}
          </button>
        ))}
      </div>
    </div>
  );
}
