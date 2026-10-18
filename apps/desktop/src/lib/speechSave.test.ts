import { describe, expect, it } from "vitest";
import { DEFAULT_SPEECH_SETTINGS, type SpeechSettings } from "./ipc";
import { createSettingsQueue } from "./settingsQueue";
import { speechPatch } from "./speechSave";

interface Fake {
  title: string;
  speech: SpeechSettings;
}

/** A store whose saves finish only when the test lets them, to make the overlap real. */
function slowStore(initial: Fake) {
  let disk = initial;
  const gates: Array<() => void> = [];
  const queue = createSettingsQueue<Fake>({
    save: (settings) =>
      new Promise((resolve) => {
        gates.push(() => {
          disk = settings;
          resolve(settings);
        });
      }),
    reset: () => Promise.resolve(initial),
  });
  queue.adopt(initial);
  return {
    queue,
    disk: () => disk,
    release: async () => {
      for (let turn = 0; turn < 20 && gates.length === 0; turn += 1) {
        await Promise.resolve();
      }
      gates.shift()?.();
    },
  };
}

const START: Fake = { title: "a", speech: DEFAULT_SPEECH_SETTINGS };

describe("saving the speech settings", () => {
  it("keeps both changes when two switches are clicked before the first save ends", async () => {
    const store = slowStore(START);

    const first = store.queue.update((current) =>
      speechPatch(current, (speech) => ({ ...speech, enabled: true })),
    );
    const second = store.queue.update((current) =>
      speechPatch(current, (speech) => ({ ...speech, notesEnabled: false })),
    );
    await store.release();
    await first;
    await store.release();
    await second;

    expect(store.disk().speech.enabled).toBe(true);
    expect(store.disk().speech.notesEnabled).toBe(false);
  });

  it("is what a top-level merge would have lost", async () => {
    const store = slowStore(START);
    // The dialog holds the copy it had when it rendered, and merges a whole `speech` object onto it.
    const stale = START.speech;

    const first = store.queue.update((current) =>
      speechPatch(current, (speech) => ({ ...speech, enabled: true })),
    );
    const second = store.queue.update({ speech: { ...stale, notesEnabled: false } });
    await store.release();
    await first;
    await store.release();
    await second;

    expect(store.disk().speech.enabled).toBe(false);
  });

  it("changes nothing else in the settings", async () => {
    const store = slowStore(START);

    const saved = store.queue.update((current) =>
      speechPatch(current, (speech) => ({ ...speech, languageOverride: "en-US" })),
    );
    await store.release();
    await saved;

    expect(store.disk().title).toBe("a");
    expect(store.disk().speech.languageOverride).toBe("en-US");
  });

  it("stores nothing when the change changes nothing", async () => {
    const store = slowStore(START);

    const result = await store.queue.update((current) => speechPatch(current, (speech) => speech));

    expect(result).toBe(START);
    expect(store.disk()).toBe(START);
  });

  it("answers null for a change that equals the current value", () => {
    expect(
      speechPatch(START, (speech) => ({ ...speech, enabled: speech.enabled })),
    ).toBeNull();
    expect(speechPatch(START, (speech) => ({ ...speech, ttsRate: 2 }))).not.toBeNull();
  });
});
