import { useCallback, useEffect, useState } from "react";
import { normaliseError, type AppError } from "../lib/errors";
import { checkServerHealth, type HealthSnapshot } from "../lib/ipc";
import { startHealthPolling } from "./healthPolling";

export type ConnectionState = "checking" | "online" | "degraded" | "offline";

export interface ServerHealthState {
  connection: ConnectionState;
  health: HealthSnapshot | null;
  error: AppError | null;
  refresh: () => void;
}

/**
 * Asks the gateway how it is: at startup, whenever the address changes, on demand, and on a timer.
 *
 * The timer matters because a practice server can go down without anybody touching this window.
 * Only a check the user asked for announces itself as "checking"; a background one changes the
 * state once it knows the answer, so the indicator does not flicker on every interval.
 */
export function useServerHealth(serverUrl: string | undefined): ServerHealthState {
  const [health, setHealth] = useState<HealthSnapshot | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("checking");

  const check = useCallback(
    (announce: boolean) => {
      if (serverUrl === undefined) {
        return Promise.resolve();
      }
      if (announce) {
        setConnection("checking");
        setError(null);
      }
      return checkServerHealth()
        .then((snapshot) => {
          setHealth(snapshot);
          setError(null);
          setConnection(snapshot.status === "ok" ? "online" : "degraded");
        })
        .catch((raw: unknown) => {
          setHealth(null);
          setError(normaliseError(raw));
          setConnection("offline");
        });
    },
    [serverUrl],
  );

  const refresh = useCallback(() => void check(true), [check]);

  useEffect(() => {
    void check(true);
    return startHealthPolling(() => check(false));
  }, [check]);

  return { connection, health, error, refresh };
}
