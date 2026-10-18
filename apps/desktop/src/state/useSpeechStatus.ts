import { useEffect, useState } from "react";
import { speechStatus, type SpeechStatus } from "../speech/ipc";

/**
 * What Rust says each speech feature can do, read when the settings block opens and again whenever
 * `refreshKey` changes (the status depends on the settings, the locale and the documents folder).
 * One cheap query that reads settings and probes: nothing is started, nothing touches the disk.
 * A reply that arrives after a newer request was made is dropped.
 */
export function useSpeechStatus(refreshKey: string): {
  status: SpeechStatus | null;
  failed: boolean;
} {
  const [status, setStatus] = useState<SpeechStatus | null>(null);
  const [failed, setFailed] = useState(false);

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
  }, [refreshKey]);

  return { status, failed };
}
