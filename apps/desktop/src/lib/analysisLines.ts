import type { Translator } from "../i18n/translate";
import {
  minutesAndSeconds,
  roundRemaining,
  type PassFamily,
  type ProgressView,
} from "./analysisProgress";
import type { IndexSummary } from "./ipc";
import { pluralise } from "./plural";

/*
 * The words of a running or finished Analyse, from the pure view of `analysisProgress.ts`: the label
 * of the gauge and the one line under the header. Catalogue keys only, so the two languages say the
 * same things.
 */

/** « 3 min 48 s » or « 5 s ». */
export function durationText(t: Translator, milliseconds: number): string {
  const { minutes, seconds } = minutesAndSeconds(milliseconds);
  return minutes > 0
    ? t("explorer.durationMinutes", { minutes, seconds })
    : t("explorer.durationSeconds", { seconds });
}

function remainingText(t: Translator, remainingMs: number | null): string {
  if (remainingMs === null) {
    return t("explorer.runEstimating");
  }
  const rounded = roundRemaining(remainingMs);
  switch (rounded.unit) {
    case "underMinute":
      return t("explorer.runRemainingUnder");
    case "minutes":
      return rounded.count === 1
        ? t("explorer.runRemainingOneMinute")
        : t("explorer.runRemainingMinutes", { count: rounded.count });
    default:
      return t("explorer.runRemainingHours", { count: rounded.count });
  }
}

/** The label inside the gauge: « Lecture 45 % », « Sens 63 % », or the start before a first message. */
export function gaugeLabel(t: Translator, view: ProgressView | null): string {
  if (view === null) {
    return t("explorer.gaugeStarting");
  }
  const key =
    view.stage === "meaning"
      ? "explorer.gaugeMeaning"
      : view.stage === "names"
        ? "explorer.gaugeNames"
        : "explorer.gaugeReading";
  return t(key, { percent: view.percent });
}

/** The name of the stage being run. */
function stageName(t: Translator, view: ProgressView, family: PassFamily): string {
  if (view.stage === "meaning") {
    return t("explorer.runMeaning");
  }
  if (view.stage === "names") {
    return t("explorer.runReadingNames");
  }
  return t(family === "documents" ? "explorer.runReadingDocuments" : "explorer.runReadingTables");
}

/**
 * The line under the header while a pass runs, stage and counts first and the times last, so that
 * when the left zone is at its narrowest it is the times that an ellipsis cuts (the full text is
 * the tooltip): « Étape 1 sur 2 · Lecture des documents · 761 / 1 245 · 3 min 48 s écoulées · environ
 * 2 min restantes ». The workbooks' pass has one stage, so it names no step.
 */
export function runLine(
  t: Translator,
  locale: string,
  family: PassFamily,
  view: ProgressView | null,
  elapsedMs: number,
): string {
  const elapsed = t("explorer.runElapsed", { time: durationText(t, view?.elapsedMs ?? elapsedMs) });
  if (view === null) {
    return [t("explorer.gaugeStarting"), elapsed].join(" · ");
  }
  const number = new Intl.NumberFormat(locale);
  const counts = t(view.stage === "meaning" ? "explorer.runPassages" : "explorer.runFiles", {
    done: number.format(view.done),
    total: number.format(view.total),
  });
  return [
    ...(view.stageCount > 1
      ? [t("explorer.runStep", { index: view.stageIndex, count: view.stageCount })]
      : []),
    stageName(t, view, family),
    counts,
    elapsed,
    remainingText(t, view.remainingMs),
  ].join(" · ");
}

/**
 * The line that replaces it once the pass is over: what was done and how long it took, or that it
 * was stopped and that what was read is kept. Counts only. A document that could not be embedded
 * counts as unreadable here exactly as an empty one does: neither can answer a question yet.
 */
export function endLine(
  t: Translator,
  locale: string,
  summary: IndexSummary,
  durationMs: number | null,
): string {
  const time = durationText(t, durationMs ?? summary.timings?.totalMs ?? 0);
  if (summary.cancelled === true) {
    return t("explorer.endStopped", { time });
  }
  const analysed = summary.indexedFiles;
  const unreadable = summary.emptyFiles.length + summary.failedFiles.length;
  if (analysed === 0 && unreadable === 0) {
    return t("explorer.endNothing", { time });
  }
  const number = new Intl.NumberFormat(locale);
  const counts = [
    ...(analysed > 0
      ? [
          `${number.format(analysed)} ${pluralise(analysed, t("workFolder.analysedOne"), t("workFolder.analysedMany"))}`,
        ]
      : []),
    ...(unreadable > 0
      ? [
          `${number.format(unreadable)} ${pluralise(unreadable, t("workFolder.unreadableOne"), t("workFolder.unreadableMany"))}`,
        ]
      : []),
  ].join(", ");
  return t("explorer.endDone", { counts, time });
}
