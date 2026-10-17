/**
 * Saves of the settings, one after the other.
 *
 * `save_settings` replaces the whole file, so two saves that start from the same copy of the
 * settings overwrite each other: the second one writes back the old value of what the first changed.
 * Each change here waits for the previous one and is merged into the settings **as they are when its
 * turn comes**, not as they were when it was asked for. A save that fails does not block the next.
 * Pure (no React, no Tauri), so the ordering is tested in the node environment.
 */

export interface SettingsQueue<S extends object> {
  /** Makes `settings` the latest known state (after a load). Meant to run inside `enqueue`. */
  adopt: (settings: S) => void;
  /** The latest settings this queue knows, `null` until one was adopted or saved. */
  latest: () => S | null;
  /** Runs `task` after everything queued before it. A rejection is the task's own: it never stops
   * the tasks queued after it. */
  enqueue: <R>(task: () => Promise<R>) => Promise<R>;
  /** Merges the change into the latest settings and stores the result. A function is given the
   * settings of its turn and may answer `null` when there is nothing to change: nothing is then
   * stored. Resolves with the settings that stand afterwards, or `null` when none are known yet.
   * Rejects when the save fails. */
  update: (change: Partial<S> | ((current: S) => Partial<S> | null)) => Promise<S | null>;
  /** Stores the defaults, after everything queued before it. Rejects when the reset fails. */
  reset: () => Promise<S>;
}

export function createSettingsQueue<S extends object>(io: {
  save: (settings: S) => Promise<S>;
  reset: () => Promise<S>;
  /** Told of every state the queue stored, in order. */
  onStored?: (settings: S) => void;
}): SettingsQueue<S> {
  let tail: Promise<unknown> = Promise.resolve();
  let latest: S | null = null;

  const enqueue = <R>(task: () => Promise<R>): Promise<R> => {
    const run = tail.then(task);
    tail = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  };

  const store = (settings: S): S => {
    latest = settings;
    io.onStored?.(settings);
    return settings;
  };

  return {
    adopt: (settings) => {
      latest = settings;
    },
    latest: () => latest,
    enqueue,
    update: (change) =>
      enqueue(async () => {
        if (latest === null) {
          return null;
        }
        const patch = typeof change === "function" ? change(latest) : change;
        if (patch === null) {
          return latest;
        }
        return store(await io.save({ ...latest, ...patch }));
      }),
    reset: () => enqueue(async () => store(await io.reset())),
  };
}
