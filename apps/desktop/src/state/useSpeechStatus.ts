import { useEffect, useState, useSyncExternalStore } from "react";
import { speechStatus, type SpeechStatus } from "../speech/ipc";
import { modelsRevision, subscribeModelsChanged } from "../speech/modelEvents";

/**
 * What Rust says each speech feature can do, read when the settings block opens and again whenever
 * `refreshKey` changes (the status depends on the settings, the locale and the documents folder).
 * It is also read again when a speech model was installed or verified, whoever asked for it, so a
 * voice installed from the settings is usable at once.
 * One cheap query that reads settings and probes: nothing is started, nothing touches the disk.
 * A reply that arrives after a newer request was made is dropped.
 */
export function useSpeechStatus(refreshKey: string): {
  status: SpeechStatus | null;
  failed: boolean;
} {
  const [status, setStatus] = useState<SpeechStatus | null>(null);
  const [failed, setFailed] = useState(false);
  const installed = useSyncExternalStore(subscribeModelsChanged, modelsRevision);

  useEffect(() => {
    let current = true;
    speechStatus().then(
      (answer) => {
        if (current) {
          setStatus(answer);
          setFailed(false);
        }
      },
      () => {
        if (current) {
          setFailed(true);
        }
      },
    );
    return () => {
      current = false;
    };
  }, [refreshKey, installed]);

  return { status, failed };
}
