import { describe, expect, it } from "vitest";
import { createSettingsQueue } from "./settingsQueue";

interface Fake {
  a: number;
  b: number;
}

/** A store whose saves finish only when the test lets them, to make the overlap real. */
function slowStore(initial: Fake) {
  let disk = initial;
  const written: Fake[] = [];
  const gates: Array<() => void> = [];
  let failNext = false;
  const queue = createSettingsQueue<Fake>({
    save: (settings) =>
      new Promise((resolve, reject) => {
        gates.push(() => {
          if (failNext) {
            failNext = false;
            reject(new Error("disk"));
            return;
          }
          disk = settings;
          written.push(settings);
          resolve(settings);
        });
      }),
    reset: () =>
      new Promise((resolve) => {
        gates.push(() => {
          disk = { a: 0, b: 0 };
          written.push(disk);
          resolve(disk);
        });
      }),
  });
  queue.adopt(initial);
  return {
    queue,
    written,
    disk: () => disk,
    failNext: () => {
      failNext = true;
    },
    /** Lets the oldest waiting save finish. */
    release: async () => {
      for (let turn = 0; turn < 20 && gates.length === 0; turn += 1) {
        await Promise.resolve();
      }
      gates.shift()?.();
    },
  };
}

describe("the settings save queue", () => {
  it("keeps both changes when two updates overlap", async () => {
    const store = slowStore({ a: 1, b: 1 });

    const first = store.queue.update({ a: 2 });
    const second = store.queue.update({ b: 2 });
    await store.release();
    await first;
    await store.release();
    await second;

    expect(store.disk()).toEqual({ a: 2, b: 2 });
    expect(store.written).toEqual([
      { a: 2, b: 1 },
      { a: 2, b: 2 },
    ]);
  });

  it("starts the second save only when the first is stored", async () => {
    const store = slowStore({ a: 1, b: 1 });

    void store.queue.update({ a: 2 });
    const second = store.queue.update({ b: 2 });
    await store.release();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(store.written).toEqual([{ a: 2, b: 1 }]);
    await store.release();
    await second;
    expect(store.written).toHaveLength(2);
  });

  it("does not let a failed save block the next one, nor swallow its own error", async () => {
    const store = slowStore({ a: 1, b: 1 });

    store.failNext();
    const first = store.queue.update({ a: 2 });
    const failure = expect(first).rejects.toThrow("disk");
    const second = store.queue.update({ b: 2 });
    await store.release();
    await failure;
    await store.release();
    await second;

    // The change that failed is not in the state the next one was merged into.
    expect(store.disk()).toEqual({ a: 1, b: 2 });
    expect(store.queue.latest()).toEqual({ a: 1, b: 2 });
  });

  it("orders a reset with the updates around it", async () => {
    const store = slowStore({ a: 1, b: 1 });

    void store.queue.update({ a: 5 });
    const reset = store.queue.reset();
    const after = store.queue.update({ b: 7 });
    await store.release();
    await store.release();
    await reset;
    await store.release();
    await after;

    expect(store.written).toEqual([{ a: 5, b: 1 }, { a: 0, b: 0 }, { a: 0, b: 7 }]);
    expect(store.disk()).toEqual({ a: 0, b: 7 });
  });

  it("merges a change made from a function against the settings of its turn", async () => {
    const store = slowStore({ a: 1, b: 1 });

    void store.queue.update({ a: 10 });
    const second = store.queue.update((current) => ({ b: current.a + 1 }));
    await store.release();
    await store.release();
    await second;

    expect(store.disk()).toEqual({ a: 10, b: 11 });
  });

  it("stores nothing when the function finds nothing to change", async () => {
    const store = slowStore({ a: 1, b: 1 });

    const unchanged = await store.queue.update(() => null);

    expect(unchanged).toEqual({ a: 1, b: 1 });
    expect(store.written).toEqual([]);
  });

  it("does nothing, and says so, while no settings are known", async () => {
    const queue = createSettingsQueue<Fake>({
      save: () => Promise.reject(new Error("never")),
      reset: () => Promise.reject(new Error("never")),
    });

    await expect(queue.update({ a: 2 })).resolves.toBeNull();
    expect(queue.latest()).toBeNull();
  });

  it("lets a load run in the same order and adopt what it read", async () => {
    const store = slowStore({ a: 1, b: 1 });
    const loaded: Fake = { a: 9, b: 9 };

    void store.queue.enqueue(async () => store.queue.adopt(loaded));
    const update = store.queue.update({ a: 3 });
    await store.release();
    await update;

    expect(store.disk()).toEqual({ a: 3, b: 9 });
  });
});
