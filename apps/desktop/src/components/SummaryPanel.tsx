import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { copyToClipboard } from "../lib/clipboard";
import { cancelSummary, readSummaries, runSummary, summaryPlan } from "../lib/ipc";
import {
  summaryStateLine,
  summaryUnitLabel,
  type StoredSummary,
  type SummaryPlan,
  type SummaryProgress,
  type SummaryRun,
  type SummaryStage,
} from "../lib/summaries";

const STAGES: SummaryStage[] = ["chapters", "full_reading"];

/** The summaries of one document, started by hand: each stage says what it will cost before it starts, shows
 * that it runs, can be stopped, and keeps what it wrote. */
export function SummaryPanel({ relativePath }: { relativePath: string }) {
  const { t } = useTranslation();
  const [plans, setPlans] = useState<Partial<Record<SummaryStage, SummaryPlan>>>({});
  const [stored, setStored] = useState<StoredSummary[]>([]);
  const [running, setRunning] = useState<SummaryStage | null>(null);
  const [progress, setProgress] = useState<SummaryProgress | null>(null);
  const [last, setLast] = useState<{ stage: SummaryStage; run: SummaryRun } | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [chapters, full, texts] = await Promise.all([
        summaryPlan(relativePath, "chapters"),
        summaryPlan(relativePath, "full_reading"),
        readSummaries(relativePath),
      ]);
      setPlans({ chapters, full_reading: full });
      setStored(texts);
    } catch {
      // The index is unavailable: nothing to offer, nothing to break.
    }
  }, [relativePath]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const start = async (stage: SummaryStage) => {
    setRunning(stage);
    setLast(null);
    setProgress(null);
    try {
      const run = await runSummary(relativePath, stage, setProgress);
      setLast({ stage, run });
    } catch {
      // A failure of the call itself leaves what was kept; the plan below shows how far it got.
    } finally {
      setRunning(null);
      setProgress(null);
      await refresh();
    }
  };

  return (
    <div className="summary">
      <p className="summary__title">{t("summary.title")}</p>
      {STAGES.map((stage) => {
        const plan = plans[stage];
        if (plan === undefined || plan.total === 0) {
          return null;
        }
        const mine = stored.filter((item) => item.stage === stage);
        const outcome =
          last !== null && last.stage === stage && running === null
            ? last.run.cancelled
              ? t("summary.stopped", { done: String(last.run.done), total: String(last.run.total) })
              : last.run.failed > 0
                ? t("summary.failed", { failed: String(last.run.failed) })
                : ""
            : "";
        return (
          <section key={stage} className="summary__stage">
            <p className="summary__name">
              {t(stage === "chapters" ? "summary.chaptersName" : "summary.fullName")}
            </p>
            <p className="summary__help">
              {t(stage === "chapters" ? "summary.chaptersHelp" : "summary.fullHelp")}
            </p>
            <p className="summary__state">{summaryStateLine(t, plan)}</p>
            {running === stage ? (
              <p className="summary__running" role="status">
                <span className="spinner" aria-hidden="true" />
                {progress === null
                  ? "…"
                  : t("summary.running", {
                      done: String(progress.done),
                      total: String(progress.total),
                      where: summaryUnitLabel(t, progress),
                    })}
                <button
                  type="button"
                  className="button button--compact"
                  onClick={() => void cancelSummary()}
                >
                  {t("summary.stop")}
                </button>
              </p>
            ) : plan.complete ? null : (
              <p>
                <button
                  type="button"
                  className="button button--compact"
                  disabled={running !== null}
                  onClick={() => void start(stage)}
                >
                  {t(plan.done > 0 ? "summary.resume" : "summary.start")}
                </button>
              </p>
            )}
            {outcome === "" ? null : <p className="summary__result">{outcome}</p>}
            {mine.length === 0 ? null : (
              <>
                <ul className="summary__list">
                  {mine.map((item) => (
                    <li key={`${stage}-${item.unit}`} className="summary__item">
                      <span className="summary__where">{summaryUnitLabel(t, item)}</span>
                      <span>{item.text}</span>
                    </li>
                  ))}
                </ul>
                <p>
                  <button
                    type="button"
                    className="button button--compact"
                    onClick={() =>
                      void copyToClipboard(
                        mine
                          .map((item) => `${summaryUnitLabel(t, item)}\n${item.text}`)
                          .join("\n\n"),
                      )
                    }
                  >
                    {t("summary.copy")}
                  </button>
                </p>
              </>
            )}
          </section>
        );
      })}
    </div>
  );
}
