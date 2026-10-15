import type { Translator } from "../i18n/translate";
import { formatDuration } from "./duration";

/** The two stages the user starts by hand (`summaries.rs`). */
export type SummaryStage = "chapters" | "full_reading";

/** What a stage would do for one document, read before anything starts (`SummaryPlan` in Rust). */
export interface SummaryPlan {
  total: number;
  done: number;
  remaining: number;
  estimatedSeconds: number;
  /** Summaries exist for an older version of the file or of the instruction. */
  stale: boolean;
  /** Only the full reading reads all of the text. */
  readsEverything: boolean;
  complete: boolean;
}

export interface SummaryProgress {
  done: number;
  total: number;
  title: string;
  firstPage: number;
  lastPage: number;
}

export interface SummaryRun {
  done: number;
  failed: number;
  total: number;
  cancelled: boolean;
  complete: boolean;
}

export interface StoredSummary {
  stage: SummaryStage;
  unit: number;
  title: string;
  firstPage: number;
  lastPage: number;
  text: string;
}

interface Place {
  title: string;
  firstPage: number;
  lastPage: number;
}

/** Where a part sits: its chapter and pages, or the pages alone for a document with no titles. */
export function summaryUnitLabel(t: Translator, place: Place): string {
  const pages =
    place.lastPage > place.firstPage
      ? t("summary.pages", { from: String(place.firstPage), to: String(place.lastPage) })
      : t("summary.page", { page: String(place.firstPage) });
  return place.title === "" ? pages : `${place.title} (${pages})`;
}

/**
 * What the user reads before starting a stage, and after it: the parts and the expected time, or how far it
 * got. "Read in full" is said only for a complete full reading; the chapters are never called that.
 */
export function summaryStateLine(t: Translator, plan: SummaryPlan): string {
  const total = String(plan.total);
  const lines: string[] = [];
  if (plan.complete) {
    lines.push(t(plan.readsEverything ? "summary.doneFull" : "summary.doneChapters", { total }));
  } else if (plan.done === 0) {
    lines.push(
      t("summary.estimate", { total, duration: formatDuration(plan.estimatedSeconds * 1000) }),
    );
  } else {
    lines.push(
      t("summary.estimateRest", {
        done: String(plan.done),
        total,
        duration: formatDuration(plan.estimatedSeconds * 1000),
      }),
    );
  }
  if (plan.stale) {
    lines.push(t("summary.stale"));
  }
  return lines.join(" ");
}
