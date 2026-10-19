import { useCallback, useRef, useState } from "react";
import { normaliseError } from "../lib/errors";
import { speechModelCancel, speechModelInstall, speechModelVerify } from "../speech/ipc";
import { notifyModelsChanged } from "../speech/modelEvents";
import { afterFailure, IDLE_JOB, installing, type ModelJob } from "../speech/modelJob";

/**
 * Runs the install or the verify of a speech model when a button asks, shows its progress, and
 * calls `onSettled` when it ends, whatever the outcome, so the state of the model is read again (and
 * every other reader of the speech status with it). Without an argument the job is about the
 * dictation model; with the id of a reading voice it is about that voice, and `target` says which.
 * One job at a time: a second press while one runs is ignored here and refused by Rust.
 */
export function useModelJob(onSettled: () => void): {
  job: ModelJob;
  /** The id of the voice the running or failed job is about; null for the dictation model. */
  target: string | null;
  install: (model?: string) => void;
  verify: (model?: string) => void;
  cancel: () => void;
} {
  const [job, setJob] = useState<ModelJob>(IDLE_JOB);
  const [target, setTarget] = useState<string | null>(null);
  const running = useRef(false);

  const run = useCallback(
    (model: string | undefined, start: ModelJob, work: () => Promise<void>) => {
      if (running.current) {
        return;
      }
      running.current = true;
      setTarget(model ?? null);
      setJob(start);
      work()
        .then(
          () => setJob(IDLE_JOB),
          (raw: unknown) => setJob(afterFailure(normaliseError(raw))),
        )
        .finally(() => {
          running.current = false;
          onSettled();
          notifyModelsChanged();
        });
    },
    [onSettled],
  );

  const install = useCallback(
    (model?: string) =>
      run(model, { kind: "installing", phase: "download", percent: 0 }, () =>
        speechModelInstall((progress) => setJob(installing(progress)), model),
      ),
    [run],
  );
  const verify = useCallback(
    (model?: string) => run(model, { kind: "verifying" }, () => speechModelVerify(model)),
    [run],
  );
  const cancel = useCallback(() => {
    void speechModelCancel();
  }, []);

  return { job, target, install, verify, cancel };
}
