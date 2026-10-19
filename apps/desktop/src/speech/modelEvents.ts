/**
 * A tiny shared counter that moves when a speech model was installed or verified. Every reader of
 * the speech status (the settings block, the read-aloud button, the dictation microphone) refreshes
 * when it moves, so a voice installed from the settings is usable at once and not after a restart.
 * It holds a number and nothing else; it starts no job and reads no file.
 */

let revision = 0;
const listeners = new Set<() => void>();

export function modelsRevision(): number {
  return revision;
}

/** Called when an install or a verify ended, whatever its outcome: the files may have changed. */
export function notifyModelsChanged(): void {
  revision += 1;
  listeners.forEach((listener) => listener());
}

export function subscribeModelsChanged(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
