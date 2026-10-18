import { describe, expect, it, vi } from "vitest";
import type { ReadingItem, ReadingPart } from "../ipc";
import { createVoiceSession } from "../voiceSession";
import { SpeechOutputError, type ReadAloudEngine, type SpeakOptions, type SpeakSegment } from "./engine";
import { noteKey, NOTE_CODES, PAUSE_MS, spokenReading } from "./pauses";
import {
  createReadAloud,
  escapeStopsReading,
  leaveEntry,
  NOTHING_TO_READ_KEY,
  type ReadAloudDeps,
} from "./readAloud";

const PARTS: ReadingPart[] = [{ kind: "markdown", text: "Bonjour." }];
const SPEECH: ReadingItem[] = [
  { kind: "speech", text: "Bonjour.", pauseAfter: "sentence" },
  { kind: "speech", text: "Au revoir.", pauseAfter: "paragraph" },
];

/** An engine that never ends a reading by itself: the test ends it with `finish`. */
function fakeEngine() {
  const calls: Array<{ segments: SpeakSegment[]; options: SpeakOptions }> = [];
  let ends: Array<() => void> = [];
  let failWith: Error | null = null;
  let active = 0;
  let mostAtOnce = 0;
  const engine: ReadAloudEngine = {
    id: "system",
    voices: async () => [],
    speak: (segments, options) =>
      new Promise<void>((resolve, reject) => {
        calls.push({ segments, options });
        active += 1;
        mostAtOnce = Math.max(mostAtOnce, active);
        if (failWith !== null) {
          active -= 1;
          reject(failWith);
          return;
        }
        ends.push(() => {
          active -= 1;
          resolve();
        });
      }),
    stop: vi.fn(() => {
      const pending = ends;
      ends = [];
      pending.forEach((end) => end());
    }),
  };
  return {
    engine,
    calls,
    finish: () => {
      const pending = ends;
      ends = [];
      pending.forEach((end) => end());
    },
    failNextWith: (error: Error) => {
      failWith = error;
    },
    mostAtOnce: () => mostAtOnce,
  };
}

function setup(overrides: Partial<ReadAloudDeps> = {}) {
  const world = fakeEngine();
  const session = createVoiceSession();
  const prepare = vi.fn(async () => SPEECH);
  const deps: ReadAloudDeps = {
    prepare,
    engine: () => world.engine,
    session,
    options: () => ({ rate: 1.1, voiceId: "paul" }),
    noteText: (code, count) => `${code}:${count}`,
    ...overrides,
  };
  return { readAloud: createReadAloud(deps), world, session, prepare };
}

const flush = async () => {
  for (let index = 0; index < 10; index += 1) {
    await Promise.resolve();
  }
};

describe("speaking_never_starts_without_a_click", () => {
  it("builds, subscribes and reads its state without preparing, claiming or speaking", () => {
    const { readAloud, world, session, prepare } = setup();

    readAloud.subscribe(() => {});
    readAloud.getSnapshot();
    readAloud.stop();
    readAloud.stopFor("a1");
    readAloud.dismissProblem("a1");

    expect(prepare).not.toHaveBeenCalled();
    expect(world.calls).toHaveLength(0);
    expect(session.holder()).toBeNull();
    expect(readAloud.getSnapshot()).toStrictEqual({
      entryId: null,
      phase: "idle",
      progress: null,
      problem: null,
    });
  });
});

describe("reading an answer", () => {
  it("prepares the parts with the locale, claims the session and speaks the items in order", async () => {
    const { readAloud, world, session, prepare } = setup();

    const reading = readAloud.start("a1", PARTS, "fr-FR");
    await flush();

    expect(prepare).toHaveBeenCalledWith(PARTS, "fr-FR");
    expect(session.holder()).toBe("readAloud");
    expect(readAloud.getSnapshot()).toMatchObject({ entryId: "a1", phase: "speaking" });
    expect(world.calls).toHaveLength(1);
    expect(world.calls[0]?.segments).toStrictEqual([
      { text: "Bonjour.", pauseAfterMs: PAUSE_MS.sentence },
      { text: "Au revoir.", pauseAfterMs: PAUSE_MS.paragraph },
    ]);
    expect(world.calls[0]?.options).toMatchObject({ locale: "fr-FR", rate: 1.1, voiceId: "paul" });

    world.finish();
    await reading;
    expect(readAloud.getSnapshot().phase).toBe("idle");
    expect(session.holder()).toBeNull();
  });

  it("follows the progress the engine reports", async () => {
    const { readAloud, world } = setup();

    void readAloud.start("a1", PARTS, "fr-FR");
    await flush();
    world.calls[0]?.options.onProgress?.({ done: 1, total: 2 });

    expect(readAloud.getSnapshot().progress).toStrictEqual({ done: 1, total: 2 });
  });

  it("one_voice_at_a_time: reading another answer ends the first", async () => {
    const { readAloud, world } = setup();

    void readAloud.start("a1", PARTS, "fr-FR");
    await flush();
    void readAloud.start("a2", PARTS, "fr-FR");
    await flush();

    expect(world.mostAtOnce()).toBe(1);
    expect(world.calls).toHaveLength(2);
    expect(readAloud.getSnapshot()).toMatchObject({ entryId: "a2", phase: "speaking" });
  });

  it("stop ends the reading, releases the session and leaves no sentence", async () => {
    const { readAloud, world, session } = setup();

    const reading = readAloud.start("a1", PARTS, "fr-FR");
    await flush();
    readAloud.stop();
    await reading;

    expect(world.engine.stop).toHaveBeenCalled();
    expect(session.holder()).toBeNull();
    expect(readAloud.getSnapshot()).toMatchObject({ entryId: null, phase: "idle", problem: null });
  });

  it("stop_on_unmount: a button that leaves ends its own reading and only its own", async () => {
    const { readAloud } = setup();

    void readAloud.start("a1", PARTS, "fr-FR");
    await flush();
    leaveEntry(readAloud, "a2");
    expect(readAloud.getSnapshot().phase).toBe("speaking");
    leaveEntry(readAloud, "a1");

    expect(readAloud.getSnapshot()).toMatchObject({ entryId: null, phase: "idle" });
  });

  it("a button that leaves also forgets the sentence it showed", async () => {
    const { readAloud } = setup({ engine: () => null });

    await readAloud.start("a1", PARTS, "fr-FR");
    expect(readAloud.getSnapshot().problem?.entryId).toBe("a1");
    leaveEntry(readAloud, "a1");

    expect(readAloud.getSnapshot().problem).toBeNull();
  });

  it("stopping while the text is being prepared means nothing is ever spoken", async () => {
    let release: (items: ReadingItem[]) => void = () => {};
    const prepare = vi.fn(() => new Promise<ReadingItem[]>((resolve) => (release = resolve)));
    const { readAloud, world, session } = setup({ prepare });

    const reading = readAloud.start("a1", PARTS, "fr-FR");
    await flush();
    expect(readAloud.getSnapshot().phase).toBe("preparing");
    readAloud.stop();
    release(SPEECH);
    await reading;

    expect(world.calls).toHaveLength(0);
    expect(session.holder()).toBeNull();
    expect(readAloud.getSnapshot().phase).toBe("idle");
  });
});

describe("what refuses a reading", () => {
  it("no engine gives tts_no_local_voice and prepares nothing", async () => {
    const { readAloud, prepare, session } = setup({ engine: () => null });

    await readAloud.start("a1", PARTS, "fr-FR");

    expect(prepare).not.toHaveBeenCalled();
    expect(session.holder()).toBeNull();
    expect(readAloud.getSnapshot().problem).toStrictEqual({
      entryId: "a1",
      key: "errors.tts_no_local_voice",
      values: { locale: "fr-FR" },
    });
  });

  it("is refused while something is recording, with the recorder's sentence, and never speaks", async () => {
    const { readAloud, world, session } = setup();
    session.claim("dictation", () => {});

    await readAloud.start("a1", PARTS, "fr-FR");

    expect(world.calls).toHaveLength(0);
    expect(readAloud.getSnapshot().problem?.key).toBe("speech.session.recording");
    expect(session.holder()).toBe("dictation");
  });

  it("a recording that starts ends the reading", async () => {
    const { readAloud, world, session } = setup();

    void readAloud.start("a1", PARTS, "fr-FR");
    await flush();
    expect(session.claim("notes", () => {})).toStrictEqual({ granted: true });

    expect(world.engine.stop).toHaveBeenCalled();
    expect(readAloud.getSnapshot()).toMatchObject({ entryId: null, phase: "idle" });
    expect(session.holder()).toBe("notes");
  });

  it("keeps the code Rust refused with, and the language it was asked for", async () => {
    const prepare = vi.fn(async () => {
      throw { code: "speech_locale_unsupported", data: {} };
    });
    const { readAloud, world } = setup({ prepare });

    await readAloud.start("a1", PARTS, "de-DE");

    expect(world.calls).toHaveLength(0);
    expect(readAloud.getSnapshot().problem).toStrictEqual({
      entryId: "a1",
      key: "errors.speech_locale_unsupported",
      values: { locale: "de-DE" },
    });
  });

  it("an unknown failure of the preparation is the general failure, never its own message", async () => {
    const prepare = vi.fn(async () => {
      throw new Error("an English exception");
    });
    const { readAloud } = setup({ prepare });

    await readAloud.start("a1", PARTS, "fr-FR");

    expect(readAloud.getSnapshot().problem?.key).toBe("errors.tts_failed");
  });

  it("a reading with no speech item does not start and says so", async () => {
    const prepare = vi.fn(async (): Promise<ReadingItem[]> => [
      { kind: "note", code: "table_omitted", count: 12 },
    ]);
    const { readAloud, world, session } = setup({ prepare });

    await readAloud.start("a1", PARTS, "fr-FR");

    expect(world.calls).toHaveLength(0);
    expect(session.holder()).toBeNull();
    expect(readAloud.getSnapshot().problem?.key).toBe(NOTHING_TO_READ_KEY);
  });

  it("an empty result gives the same sentence", async () => {
    const { readAloud, world } = setup({ prepare: vi.fn(async () => []) });

    await readAloud.start("a1", PARTS, "fr-FR");

    expect(world.calls).toHaveLength(0);
    expect(readAloud.getSnapshot().problem?.key).toBe(NOTHING_TO_READ_KEY);
  });

  it("a failure of the engine is shown and frees the voice", async () => {
    const { readAloud, world, session } = setup();
    world.failNextWith(new SpeechOutputError("tts_failed"));

    await readAloud.start("a1", PARTS, "fr-FR");

    expect(readAloud.getSnapshot().problem?.key).toBe("errors.tts_failed");
    expect(session.holder()).toBeNull();
    expect(readAloud.getSnapshot().phase).toBe("idle");
  });

  it("a new reading forgets the previous sentence", async () => {
    const world = fakeEngine();
    let available = false;
    const { readAloud } = setup({ engine: () => (available ? world.engine : null) });
    await readAloud.start("a1", PARTS, "fr-FR");
    expect(readAloud.getSnapshot().problem).not.toBeNull();

    available = true;
    void readAloud.start("a1", PARTS, "fr-FR");
    await flush();

    expect(readAloud.getSnapshot().problem).toBeNull();
    expect(readAloud.getSnapshot().phase).toBe("speaking");
  });
});

describe("what is spoken", () => {
  it("speaks a note where it stands, and rests as long as a paragraph after it", () => {
    const reading = spokenReading(
      [
        { kind: "speech", text: "Avant.", pauseAfter: "clause" },
        { kind: "note", code: "table_omitted", count: 12 },
        { kind: "speech", text: "Apres.", pauseAfter: "sentence" },
      ],
      (code, count) => `${code} ${count}`,
    );

    expect(reading.hasSpeech).toBe(true);
    expect(reading.segments).toStrictEqual([
      { text: "Avant.", pauseAfterMs: PAUSE_MS.clause },
      { text: "table_omitted 12", pauseAfterMs: PAUSE_MS.paragraph },
      { text: "Apres.", pauseAfterMs: PAUSE_MS.sentence },
    ]);
  });

  it("does not count a blank item as speech", () => {
    const reading = spokenReading(
      [{ kind: "speech", text: "   ", pauseAfter: "sentence" }],
      () => "",
    );

    expect(reading).toStrictEqual({ segments: [], hasSpeech: false });
  });

  it("rests longer after a paragraph than after a sentence, and after a sentence than inside one", () => {
    expect(PAUSE_MS.clause).toBeLessThan(PAUSE_MS.sentence);
    expect(PAUSE_MS.sentence).toBeLessThan(PAUSE_MS.paragraph);
  });

  it("has a catalogue key for every note code", () => {
    expect(NOTE_CODES.map(noteKey)).toStrictEqual([
      "speech.readAloudNote.table_omitted",
      "speech.readAloudNote.code_omitted",
    ]);
  });
});

describe("Escape", () => {
  it("stops a reading in progress when no dialog is open, and nothing else", () => {
    expect(escapeStopsReading("Escape", "speaking", false)).toBe(true);
    expect(escapeStopsReading("Escape", "preparing", false)).toBe(true);
    expect(escapeStopsReading("Escape", "idle", false)).toBe(false);
    expect(escapeStopsReading("Escape", "speaking", true)).toBe(false);
    expect(escapeStopsReading("Enter", "speaking", false)).toBe(false);
  });
});
