import type { SpeechSettings } from "./ipc";

/**
 * The patch that stores a change to the nested `speech` settings. The save queue merges only
 * top-level keys, so `update({ speech: { enabled: true } })` would replace every other speech field
 * with the copy the dialog holds, which may be stale after a quick second click. Handing the queue
 * a function instead means the change is applied to the speech settings **as they stand when its
 * turn comes**. `null` when nothing would change: nothing is then stored.
 */
export function speechPatch(
  current: { speech: SpeechSettings },
  change: (current: SpeechSettings) => SpeechSettings,
): { speech: SpeechSettings } | null {
  const next = change(current.speech);
  const keys = Object.keys(next) as Array<keyof SpeechSettings>;
  const same = keys.every((key) => next[key] === current.speech[key]);
  return same ? null : { speech: next };
}
