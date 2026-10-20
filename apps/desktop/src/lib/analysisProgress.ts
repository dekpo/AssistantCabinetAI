import type { IndexProgress } from "./ipc";

/*
 * What the gauge of a running Analyse says: the stage, its percentage, the files read, the time
 * elapsed and an honest time remaining. A pure module: the clock is always an input (a sample's
 * `at`, a view's `now`), never read here, so every rule below is tested with synthetic numbers.
 *
 * The remaining time (owner, 18 October 2026) is returned only after 10 % of the stage and 8 s in
 * it, is computed from the recent rate rather than the overall average (a scan costs far more than
 * a text file) and is smoothed so it does not jump. It is the time left in the stage being run: the
 * length of the next one is not known before it starts.
 */

export type ProgressStage = "reading" | "names" | "meaning";

/** Which pass the gauge belongs to: documents have two stages, the workbooks one. */
export type PassFamily = "documents" | "tables";

export interface ProgressSample {
  /** The client's clock when the progress arrived, in milliseconds. */
  at: number;
  progress: IndexProgress;
}

/** The share of a stage that must be done before a remaining time is shown. */
export const MIN_SHARE_FOR_ESTIMATE = 0.1;
/** The time a stage must have been running before a remaining time is shown. */
export const MIN_STAGE_MS_FOR_ESTIMATE = 8000;
/** The rate is measured over the last samples spanning at least this long (and at least two). */
export const RATE_WINDOW_MS = 15000;
/** How much of each new estimate of the finishing time is taken into the smoothed one. */
export const SMOOTHING = 0.3;
/** A share that goes back by more than this is a stage that started over; a smaller step back is
 * jitter (a batch counted late) and is read as no progress, never as a restart. */
const RESTART_DROP = 0.02;
/** Two samples closer than this are one: the newer replaces the older, so a pass over small text
 * files (many per second) does not grow the series without bound. */
const MIN_SAMPLE_GAP_MS = 250;

/** What the pass has reported so far, small enough to rebuild a view from at any time. */
export interface ProgressTrace {
  /** When the pass started (the user pressed Analyser), not when its first progress arrived. */
  startedAt: number;
  stage: ProgressStage | null;
  /** When the stage being run was first seen. */
  stageStartedAt: number;
  /** The last samples of this stage, thinned, at least two once there are two. */
  recent: ProgressSample[];
  /** The smoothed moment the stage will be over, or null while it cannot be told. */
  finishAt: number | null;
}

export interface ProgressView {
  stage: ProgressStage;
  /** 1 or 2: the names being read again is still the first stage. */
  stageIndex: number;
  stageCount: number;
  /** Whole percent of the stage, never above 100. */
  percent: number;
  /** What the stage counts: files read, documents read again, or passages with a vector. */
  done: number;
  total: number;
  elapsedMs: number;
  /** Null until the gate (10 % and 8 s) is open and a rate can be measured. */
  remainingMs: number | null;
}

export function initialTrace(startedAt: number): ProgressTrace {
  return { startedAt, stage: null, stageStartedAt: startedAt, recent: [], finishAt: null };
}

export function stageOf(progress: IndexProgress): ProgressStage {
  if ((progress.vectorsTotal ?? 0) > 0) {
    return "meaning";
  }
  return progress.readingNames === true ? "names" : "reading";
}

/** The share of the stage done, between 0 and 1. A stage with nothing to do is done. Inside a long
 * document the share moves by its batches, so one big file does not look frozen. */
export function stageFraction(progress: IndexProgress): number {
  let fraction: number;
  switch (stageOf(progress)) {
    case "meaning":
      fraction = (progress.vectorsDone ?? 0) / (progress.vectorsTotal ?? 1);
      break;
    case "names":
      fraction = progress.totalFiles <= 0 ? 1 : progress.processedFiles / progress.totalFiles;
      break;
    default: {
      const inside =
        progress.batchTotal > 0 ? Math.min(1, progress.batchIndex / progress.batchTotal) : 0;
      fraction =
        progress.totalFiles <= 0 ? 1 : (progress.processedFiles + inside) / progress.totalFiles;
    }
  }
  return Math.min(1, Math.max(0, fraction));
}

function counts(progress: IndexProgress): { done: number; total: number } {
  if (stageOf(progress) === "meaning") {
    const total = progress.vectorsTotal ?? 0;
    return { done: Math.min(total, progress.vectorsDone ?? 0), total };
  }
  return {
    done: Math.min(progress.totalFiles, progress.processedFiles),
    total: progress.totalFiles,
  };
}

/** Take one progress message into the trace. */
export function stepTrace(trace: ProgressTrace, sample: ProgressSample): ProgressTrace {
  const stage = stageOf(sample.progress);
  const fraction = stageFraction(sample.progress);
  const last = trace.recent[trace.recent.length - 1];
  // A new stage, or a share that went back (the stage started over): nothing measured before it
  // describes this run, so its estimate and its clock start again.
  if (
    trace.stage !== stage ||
    last === undefined ||
    fraction < stageFraction(last.progress) - RESTART_DROP
  ) {
    return {
      ...trace,
      stage,
      stageStartedAt: sample.at,
      recent: [sample],
      finishAt: null,
    };
  }

  const recent = trace.recent.slice();
  const beforeLast = recent[recent.length - 2];
  if (beforeLast !== undefined && sample.at - beforeLast.at < MIN_SAMPLE_GAP_MS) {
    recent[recent.length - 1] = sample;
  } else {
    recent.push(sample);
  }
  while (recent.length > 2 && sample.at - (recent[1]?.at ?? sample.at) >= RATE_WINDOW_MS) {
    recent.shift();
  }

  return {
    ...trace,
    recent,
    finishAt: nextFinishAt(trace, recent, sample, fraction),
  };
}

function nextFinishAt(
  trace: ProgressTrace,
  recent: readonly ProgressSample[],
  sample: ProgressSample,
  fraction: number,
): number | null {
  if (fraction >= 1) {
    return sample.at;
  }
  if (
    fraction < MIN_SHARE_FOR_ESTIMATE ||
    sample.at - trace.stageStartedAt < MIN_STAGE_MS_FOR_ESTIMATE
  ) {
    return null;
  }
  const oldest = recent[0];
  if (oldest === undefined) {
    return null;
  }
  const span = sample.at - oldest.at;
  const gained = fraction - stageFraction(oldest.progress);
  // Nothing moved over the window (a very slow scan, a stalled server): no honest estimate.
  if (span <= 0 || gained <= 0) {
    return null;
  }
  const estimate = sample.at + ((1 - fraction) * span) / gained;
  return trace.finishAt === null ? estimate : trace.finishAt + SMOOTHING * (estimate - trace.finishAt);
}

/** The trace of a series of samples, oldest first. */
export function traceOf(startedAt: number, samples: readonly ProgressSample[]): ProgressTrace {
  return samples.reduce(stepTrace, initialTrace(startedAt));
}

/** What the gauge shows at `now`, or null while the pass has reported nothing yet. */
export function progressView(
  trace: ProgressTrace,
  now: number,
  family: PassFamily,
): ProgressView | null {
  const latest = trace.recent[trace.recent.length - 1];
  if (trace.stage === null || latest === undefined) {
    return null;
  }
  const fraction = stageFraction(latest.progress);
  const gateOpen =
    fraction >= MIN_SHARE_FOR_ESTIMATE && now - trace.stageStartedAt >= MIN_STAGE_MS_FOR_ESTIMATE;
  const { done, total } = counts(latest.progress);
  return {
    stage: trace.stage,
    stageIndex: trace.stage === "meaning" ? 2 : 1,
    stageCount: family === "documents" ? 2 : 1,
    percent: Math.min(100, Math.floor(100 * fraction)),
    done,
    total,
    elapsedMs: Math.max(0, now - trace.startedAt),
    remainingMs:
      gateOpen && trace.finishAt !== null ? Math.max(0, trace.finishAt - now) : null,
  };
}

/** A time remaining as the words say it: under a minute, a number of minutes, or of hours. */
export type RemainingTime =
  | { unit: "underMinute" }
  | { unit: "minutes"; count: number }
  | { unit: "hours"; count: number };

/** Rounded the way it is said: « moins d'une minute », « environ N min », « environ N h ». */
export function roundRemaining(milliseconds: number): RemainingTime {
  const seconds = Math.max(0, milliseconds) / 1000;
  if (seconds < 60) {
    return { unit: "underMinute" };
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 90) {
    return { unit: "minutes", count: minutes };
  }
  return { unit: "hours", count: Math.round(minutes / 60) };
}

/** Whole minutes and seconds of a time elapsed, for « 3 min 48 s ». */
export function minutesAndSeconds(milliseconds: number): { minutes: number; seconds: number } {
  const total = Math.max(0, Math.round(milliseconds / 1000));
  return { minutes: Math.floor(total / 60), seconds: total % 60 };
}
