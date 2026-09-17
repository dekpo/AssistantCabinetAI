import { useCallback, useEffect, useState } from "react";
import { normaliseError, type AppError } from "../lib/errors";
import { checkServerHealth, type HealthSnapshot } from "../lib/ipc";

export type ConnectionState = "checking" | "online" | "degraded" | "offline";

export interface ServerHealthState {
  connection: ConnectionState;
  health: HealthSnapshot | null;
  error: AppError | null;
  refresh: () => void;
}

/** Asks the gateway how it is, and re-asks whenever the address it lives at changes. */
export function useServerHealth(serverUrl: string | undefined): ServerHealthState {
  const [health, setHealth] = useState<HealthSnapshot | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("checking");

  const refresh = useCallback(() => {
    if (serverUrl === undefined) {
      return;
    }
    setConnection("checking");
    setError(null);
    checkServerHealth()
      .then((snapshot) => {
        setHealth(snapshot);
        setConnection(snapshot.status === "ok" ? "online" : "degraded");
      })
      .catch((raw: unknown) => {
        setHealth(null);
        setError(normaliseError(raw));
        setConnection("offline");
      });
  }, [serverUrl]);

  useEffect(refresh, [refresh]);

  return { connection, health, error, refresh };
}
