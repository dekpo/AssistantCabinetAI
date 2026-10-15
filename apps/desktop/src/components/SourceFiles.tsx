import { useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { sourceGroupLine } from "../lib/annotations";
import { revealWorkFile, type Evidence } from "../lib/ipc";
import { groupSources } from "../lib/sourceGroups";
import { SummaryPanel } from "./SummaryPanel";

/** The documents an answer drew from, one line each with its pages and sections, and the two things she
 * can do with a document from here: summarise it (started by hand) and show the file. */
export function SourceFiles({ sources }: { sources: Evidence[] }) {
  const { t } = useTranslation();
  // The document whose summaries are open: they are started by hand, from here.
  const [summarising, setSummarising] = useState<string | null>(null);

  return (
    <ul className="message__sources-list">
      {groupSources(sources).map((group) => (
        <li key={group.relativePath} className="message__source">
          <div className="message__source-row">
            <span className="message__source-line">{sourceGroupLine(t, group)}</span>
            <span className="message__source-buttons">
              <button
                type="button"
                className="button button--compact"
                onClick={() =>
                  setSummarising(summarising === group.relativePath ? null : group.relativePath)
                }
              >
                {t("summary.button")}
              </button>
              <button
                type="button"
                className="button button--compact"
                onClick={() => void revealWorkFile(group.relativePath)}
              >
                {t("sources.show")}
              </button>
            </span>
          </div>
          {summarising === group.relativePath ? (
            <SummaryPanel relativePath={group.relativePath} />
          ) : null}
        </li>
      ))}
    </ul>
  );
}
