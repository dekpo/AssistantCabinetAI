import { useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { aggregateRows, answerDiagnosticRows } from "../lib/diagnostics";
import {
  knowledgeDiagnostics,
  type KnowledgeAggregates,
  type RetrievalDiagnostics,
} from "../lib/ipc";

/** How many recorded questions the aggregate view adds up. */
const AGGREGATE_WINDOW = 500;

/**
 * The developer panel, under the last answer, only when `showDiagnostics` is on. It lists where the
 * last question spent its time and what the knowledge base did, then the recorded questions added
 * up. Numbers and machine codes only: no question, no name and no file name is ever in it, so it
 * can be photographed for a bug report.
 *
 * The aggregate is one cheap query, read when the panel appears and after each new answer; nothing
 * runs on a timer.
 */
export function DiagnosticsPanel({
  diagnostics,
}: {
  diagnostics: RetrievalDiagnostics | undefined;
}) {
  const { t } = useTranslation();
  const [totals, setTotals] = useState<KnowledgeAggregates | null>(null);

  useEffect(() => {
    let current = true;
    knowledgeDiagnostics(AGGREGATE_WINDOW).then(
      (found) => {
        if (current) {
          setTotals(found);
        }
      },
      () => {
        if (current) {
          setTotals(null);
        }
      },
    );
    return () => {
      current = false;
    };
  }, [diagnostics]);

  const answerRows = diagnostics === undefined ? [] : answerDiagnosticRows(t, diagnostics);
  const totalRows = totals === null ? [] : aggregateRows(t, totals);

  return (
    <details className="diagnostics">
      <summary className="disclosure">{t("diagnostics.title")}</summary>
      <h3 className="diagnostics__heading">{t("diagnostics.thisAnswer")}</h3>
      {answerRows.length === 0 ? (
        <p className="message__timing">{t("diagnostics.noneForAnswer")}</p>
      ) : (
        <dl className="diagnostics__rows">
          {answerRows.map((row) => (
            <div className="diagnostics__row" key={row.label}>
              <dt>{row.label}</dt>
              <dd>{row.value}</dd>
            </div>
          ))}
        </dl>
      )}
      <h3 className="diagnostics__heading">{t("diagnostics.recorded")}</h3>
      {totalRows.length === 0 ? (
        <p className="message__timing">{t("diagnostics.noneRecorded")}</p>
      ) : (
        <dl className="diagnostics__rows">
          {totalRows.map((row) => (
            <div className="diagnostics__row" key={row.label}>
              <dt>{row.label}</dt>
              <dd>{row.value}</dd>
            </div>
          ))}
        </dl>
      )}
    </details>
  );
}
