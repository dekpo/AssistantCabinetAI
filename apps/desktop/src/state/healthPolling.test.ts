import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HEALTH_POLL_INTERVAL_MS, startHealthPolling } from "./healthPolling";

describe("health polling", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("asks again on every interval", async () => {
    const check = vi.fn(() => Promise.resolve());
    const stop = startHealthPolling(check, 1000);

    await vi.advanceTimersByTimeAsync(3000);
    stop();

    expect(check).toHaveBeenCalledTimes(3);
  });

  it("waits rather than piling requests on a gateway that stopped answering", async () => {
    let release = () => {};
    const check = vi.fn(() => new Promise<void>((resolve) => (release = resolve)));
    const stop = startHealthPolling(check, 1000);

    await vi.advanceTimersByTimeAsync(5000);
    expect(check).toHaveBeenCalledTimes(1);

    release();
    await vi.advanceTimersByTimeAsync(1000);
    stop();

    expect(check).toHaveBeenCalledTimes(2);
  });

  it("stops when told to, so a closed window asks nothing", async () => {
    const check = vi.fn(() => Promise.resolve());
    const stop = startHealthPolling(check, 1000);

    await vi.advanceTimersByTimeAsync(1000);
    stop();
    await vi.advanceTimersByTimeAsync(5000);

    expect(check).toHaveBeenCalledTimes(1);
  });

  it("keeps the interval far enough apart to be invisible on the practice network", () => {
    expect(HEALTH_POLL_INTERVAL_MS).toBeGreaterThanOrEqual(10_000);
  });
});
