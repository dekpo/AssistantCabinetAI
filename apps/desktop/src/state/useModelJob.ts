import { useCallback, useRef, useState } from "react";
import { normaliseError } from "../lib/errors";
import { speechModelCancel, speechModelInstall, speechModelVerify } from "../speech/ipc";
import { afterFailure, IDLE_JOB, installing, type ModelJob } from "../speech/modelJob";

/**
 * Runs the install or the verify of the speech model when a button asks, shows its progress, and
 * calls `onSettled` when it ends, whatever the outcome, so the state of the model is read again.
 * One job at a time: a second press while one runs is ignored here and refused by Rust.
 */
export function useModelJob(onSettled: () => void): {
  job: ModelJob;
  install: () => void;
  verify: () => void;
  cancel: () => void;
} {
  const [job, setJob] = useState<ModelJob>(IDLE_JOB);
  const running = useRef(false);

  const run = useCallback(
    (start: ModelJob, work: () => Promise<void>) => {
      if (running.current) {
        return;
      }
      running.current = true;
      setJob(start);
      work()
        .then(
          () => setJob(IDLE_JOB),
          (raw: unknown) => setJob(afterFailure(normaliseError(raw))),
        )
        .finally(() => {
          running.current = false;
          onSettled();
        });
    },
    [onSettled],
  );

  const install = useCallback(
    () =>
      run({ kind: "installing", phase: "download", percent: 0 }, () =>
        speechModelInstall((progress) => setJob(installing(progress))),
      ),
    [run],
  );
  const verify = useCallback(() => run({ kind: "verifying" }, speechModelVerify), [run]);
  const cancel = useCallback(() => {
    void speechModelCancel();
  }, []);

  return { job, install, verify, cancel };
}
