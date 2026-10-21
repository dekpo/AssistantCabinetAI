import { useTranslation } from "../../i18n/I18nProvider";
import type { CentreSection } from "../../lib/centreSection";

export const centreTabId = (section: CentreSection) => `centre-tab-${section}`;
export const centrePanelId = (section: CentreSection) => `centre-panel-${section}`;

/**
 * The switch at the top of the centre zone, drawn whenever the zone is open: Aperçu, Fichiers and,
 * only while its own switch is on, Notes. Switching is never a way to lose a note, so it asks
 * nothing. A red dot on Notes says a recording is running while another section is on screen.
 */
export function CentreSwitch({
  sections,
  section,
  recording = false,
  onChange,
}: {
  /** The sections on offer, in order (`offeredSections`). */
  sections: readonly CentreSection[];
  section: CentreSection;
  recording?: boolean;
  onChange: (section: CentreSection) => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="centre__switch" role="tablist" aria-label={t("centre.switchLabel")}>
      {sections.map((entry) => (
        <button
          key={entry}
          type="button"
          role="tab"
          id={centreTabId(entry)}
          className={entry === section ? "centre__tab centre__tab--active" : "centre__tab"}
          aria-selected={entry === section}
          aria-controls={centrePanelId(entry)}
          onClick={() => onChange(entry)}
        >
          {t(`centre.${entry}`)}
          {entry === "notes" && recording ? (
            <span className="centre__dot" role="img" aria-label={t("centre.recording")} />
          ) : null}
        </button>
      ))}
    </div>
  );
}
