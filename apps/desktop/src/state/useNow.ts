import { useEffect, useState } from "react";

/**
 * The clock, ticking once a second while `active`, so a time elapsed keeps counting between two
 * progress messages (a scan can take half a minute without sending one). Idle, it does not tick and
 * schedules nothing.
 */
export function useNow(active: boolean, intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) {
      return undefined;
    }
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [active, intervalMs]);
  return now;
}
