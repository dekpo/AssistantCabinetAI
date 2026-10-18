import { useTranslation } from "../../i18n/I18nProvider";
import type { CentreSection } from "../../lib/centreSection";

const SECTIONS: ReadonlyArray<{ section: CentreSection; label: string }> = [
  { section: "files", label: "centre.files" },
  { section: "notes", label: "centre.notes" },
];

export const centreTabId = (section: CentreSection) => `centre-tab-${section}`;
export const centrePanelId = (section: CentreSection) => `centre-panel-${section}`;

/**
 * The two-position switch at the top of the centre zone. Shown only while the Notes section exists;
 * switching is never a way to lose a note, so it asks nothing. A red dot on Notes says a recording
 * is running while Files is on screen.
 */
export function CentreSwitch({
  section,
  recording = false,
  onChange,
}: {
  section: CentreSection;
  recording?: boolean;
  onChange: (section: CentreSection) => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="centre__switch" role="tablist" aria-label={t("centre.switchLabel")}>
      {SECTIONS.map((entry) => (
        <button
          key={entry.section}
          type="button"
          role="tab"
          id={centreTabId(entry.section)}
          className={
            entry.section === section ? "centre__tab centre__tab--active" : "centre__tab"
          }
          aria-selected={entry.section === section}
          aria-controls={centrePanelId(entry.section)}
          onClick={() => onChange(entry.section)}
        >
          {t(entry.label)}
          {entry.section === "notes" && recording ? (
            <span className="centre__dot" role="img" aria-label={t("centre.recording")} />
          ) : null}
        </button>
      ))}
    </div>
  );
}
