import type { AppError } from "../lib/errors";
import type { ModelProgress, ModelState } from "./ipc";

/**
 * The install or verify of the speech model, as the settings block shows it. Pure functions of what
 * Rust reported, so the states and the buttons are tested without a window. Nothing here starts a
 * job: only a button does.
 */

export type ModelJob =
  | { kind: "idle" }
  | { kind: "installing"; phase: ModelProgress["phase"]; percent: number }
  | { kind: "verifying" }
  | { kind: "failed"; error: AppError };

export const IDLE_JOB: ModelJob = { kind: "idle" };

/** Rust answers this when the user stopped the job: not a failure, so nothing is shown. */
export const CANCELLED_CODE = "speech_cancelled";

/** Whole percent of a report, never outside 0 to 100, and 0 while the total is not known. */
export function percentOf(progress: ModelProgress): number {
  if (progress.totalBytes <= 0) {
    return 0;
  }
  const share = Math.floor((progress.doneBytes / progress.totalBytes) * 100);
  return Math.min(100, Math.max(0, share));
}

export function installing(progress: ModelProgress): ModelJob {
  return { kind: "installing", phase: progress.phase, percent: percentOf(progress) };
}

/** What a failed job leaves on screen: a cancel leaves nothing, any other failure its error. */
export function afterFailure(error: AppError): ModelJob {
  return error.code === CANCELLED_CODE ? IDLE_JOB : { kind: "failed", error };
}

export interface JobLine {
  key: string;
  values: Record<string, number>;
}

/** The sentence of a running job, or null when none is running. */
export function jobLine(job: ModelJob): JobLine | null {
  switch (job.kind) {
    case "installing":
      return {
        key: job.phase === "download" ? "speech.install.downloading" : "speech.install.unpacking",
        values: { percent: job.percent },
      };
    case "verifying":
      return { key: "speech.install.verifying", values: {} };
    default:
      return null;
  }
}

export interface ModelButtons {
  /** Install is for a model that is not there or is damaged; a present one needs no install. */
  installDisabled: boolean;
  /** Verify hashes the files in the folder: for a model placed by hand, or to check one. */
  verifyDisabled: boolean;
  /** Cancel is shown only while a job runs. */
  cancelShown: boolean;
}

export function modelButtons(
  job: ModelJob,
  model: ModelState,
  installAvailable: boolean,
): ModelButtons {
  const running = job.kind === "installing" || job.kind === "verifying";
  return {
    installDisabled: !installAvailable || running || model === "present" || model === "unverified",
    verifyDisabled: !installAvailable || running || model === "missing",
    cancelShown: running,
  };
}
