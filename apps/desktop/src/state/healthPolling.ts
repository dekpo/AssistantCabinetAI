/** How often the window re-asks the gateway how it is, with nobody clicking anything. */
export const HEALTH_POLL_INTERVAL_MS = 20_000;

/**
 * Repeats `check` on a fixed interval and returns the function that stops it.
 *
 * A tick is skipped while the previous one is still in flight: a gateway that has stopped
 * answering must not accumulate a queue of pending checks behind it.
 */
export function startHealthPolling(
  check: () => Promise<unknown>,
  intervalMs: number = HEALTH_POLL_INTERVAL_MS,
): () => void {
  let inFlight = false;

  const timer = setInterval(() => {
    if (inFlight) {
      return;
    }
    inFlight = true;
    void Promise.resolve(check()).finally(() => {
      inFlight = false;
    });
  }, intervalMs);

  return () => clearInterval(timer);
}
