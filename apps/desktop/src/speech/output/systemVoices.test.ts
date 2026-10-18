import { describe, expect, it, vi } from "vitest";
import { SpeechOutputError } from "./engine";
import {
  createSystemVoiceEngine,
  loadVoices,
  pickLocalVoice,
  summariseVoices,
  type SynthesisLike,
  type SynthesisVoice,
  type Timers,
  type UtteranceLike,
} from "./systemVoices";

const voice = (name: string, lang: string, localService: boolean): SynthesisVoice => ({
  name,
  lang,
  voiceURI: name,
  localService,
});

const HORTENSE = voice("Hortense", "fr-FR", true);
const DENISE = voice("Denise Online (Natural)", "fr-FR", false);
const CELINE = voice("Celine", "fr-CA", true);
const ZIRA = voice("Zira", "en-US", true);

describe("which system voice is used", () => {
  it("a_non_local_voice_is_never_selected", () => {
    expect(pickLocalVoice([DENISE], "fr-FR", null)).toBeNull();
    expect(pickLocalVoice([DENISE], "fr-FR", "Denise Online (Natural)")).toBeNull();
    expect(pickLocalVoice([DENISE, HORTENSE], "fr-FR", "Denise Online (Natural)")).toBe(HORTENSE);
    // The flag must be exactly true: an absent or odd value is not "local".
    const unknown = { name: "X", lang: "fr-FR", voiceURI: "X" } as unknown as SynthesisVoice;
    expect(pickLocalVoice([unknown], "fr-FR", null)).toBeNull();
  });

  it("prefers the exact language, then the same primary language", () => {
    expect(pickLocalVoice([CELINE, HORTENSE], "fr-FR", null)).toBe(HORTENSE);
    expect(pickLocalVoice([CELINE, ZIRA], "fr-FR", null)).toBe(CELINE);
    expect(pickLocalVoice([voice("Hortense", "fr_FR", true)], "fr-fr", null)?.name).toBe("Hortense");
  });

  it("honours the preferred voice within the best group, and ignores one that is not there", () => {
    const paul = voice("Paul", "fr-FR", true);
    expect(pickLocalVoice([HORTENSE, paul], "fr-FR", "paul")).toBe(paul);
    expect(pickLocalVoice([HORTENSE, paul], "fr-FR", "nobody")).toBe(HORTENSE);
    // A preferred voice of a worse group does not beat the exact language.
    expect(pickLocalVoice([CELINE, HORTENSE], "fr-FR", "Celine")).toBe(HORTENSE);
  });

  it("is null for a language with no local voice, never a default", () => {
    expect(pickLocalVoice([ZIRA], "fr-FR", null)).toBeNull();
    expect(pickLocalVoice([], "fr-FR", null)).toBeNull();
  });

  it("summarises the local voices and counts the online ones of the language", () => {
    expect(summariseVoices([HORTENSE, DENISE, CELINE, ZIRA], "fr-FR")).toStrictEqual({
      names: ["Hortense", "Celine"],
      ignored: 1,
    });
    expect(summariseVoices([ZIRA], "fr-FR")).toStrictEqual({ names: [], ignored: 0 });
  });
});

/** A synthesis that records what it is asked, and ends an utterance only when the test says so. */
function fakeSynth(initial: SynthesisVoice[]) {
  let voices = initial;
  const listeners = new Set<() => void>();
  const spoken: Array<UtteranceLike & { text: string }> = [];
  const synth: SynthesisLike = {
    getVoices: () => voices,
    speak: vi.fn((utterance) => spoken.push(utterance as UtteranceLike & { text: string })),
    cancel: vi.fn(() => {
      const last = spoken[spoken.length - 1];
      last?.onerror?.({ error: "canceled" });
    }),
    addEventListener: (_type, listener) => void listeners.add(listener),
    removeEventListener: (_type, listener) => void listeners.delete(listener),
  };
  return {
    synth,
    spoken,
    setVoices: (next: SynthesisVoice[]) => {
      voices = next;
      listeners.forEach((listener) => listener());
    },
    listenerCount: () => listeners.size,
  };
}

/** Timers the test fires by hand. */
function fakeTimers() {
  const pending = new Map<number, { callback: () => void; ms: number }>();
  let next = 1;
  const timers: Timers = {
    set: (callback, ms) => {
      pending.set(next, { callback, ms });
      return next++;
    },
    clear: (handle) => void pending.delete(handle as number),
  };
  return {
    timers,
    pending,
    fireAll: () => {
      for (const [id, timer] of [...pending]) {
        pending.delete(id);
        timer.callback();
      }
    },
  };
}

const flush = async () => {
  for (let index = 0; index < 8; index += 1) {
    await Promise.resolve();
  }
};

function engineOver(voices: SynthesisVoice[]) {
  const world = fakeSynth(voices);
  const clock = fakeTimers();
  const engine = createSystemVoiceEngine({
    synth: world.synth,
    makeUtterance: (text) => ({ text, voice: null, lang: "", rate: 1, onend: null, onerror: null }),
    timers: clock.timers,
  });
  return { engine, ...world, clock };
}

const OPTIONS = { locale: "fr-FR", rate: 1.2, voiceId: null };

describe("the system voice engine", () => {
  it("no_local_voice_refuses: it says tts_no_local_voice and never speaks", async () => {
    const { engine, synth } = engineOver([DENISE, ZIRA]);

    const reading = engine.speak([{ text: "Bonjour.", pauseAfterMs: 100 }], OPTIONS);

    await expect(reading).rejects.toStrictEqual(new SpeechOutputError("tts_no_local_voice"));
    expect(synth.speak).not.toHaveBeenCalled();
  });

  it("speaks the segments in order with a local voice, the rate and the language, and reports progress", async () => {
    const { engine, spoken, clock } = engineOver([DENISE, HORTENSE]);
    const progress: Array<[number, number]> = [];

    const reading = engine.speak(
      [
        { text: "Un.", pauseAfterMs: 380 },
        { text: "Deux.", pauseAfterMs: 760 },
      ],
      { ...OPTIONS, onProgress: ({ done, total }) => progress.push([done, total]) },
    );
    await flush();
    expect(spoken.map((each) => each.text)).toStrictEqual(["Un."]);
    expect(spoken[0]?.voice).toBe(HORTENSE);
    expect(spoken[0]?.lang).toBe("fr-FR");
    expect(spoken[0]?.rate).toBe(1.2);

    spoken[0]?.onend?.();
    await flush();
    // The rest between the two segments is a timer, and the second one waits for it.
    expect([...clock.pending.values()].map((timer) => timer.ms)).toStrictEqual([380]);
    expect(spoken).toHaveLength(1);
    clock.fireAll();
    await flush();
    expect(spoken.map((each) => each.text)).toStrictEqual(["Un.", "Deux."]);
    spoken[1]?.onend?.();

    await reading;
    expect(progress).toStrictEqual([
      [0, 2],
      [1, 2],
      [2, 2],
    ]);
    // No rest after the last segment.
    expect(clock.pending.size).toBe(0);
  });

  it("one_voice_at_a_time: a new reading ends the one in progress", async () => {
    const { engine, spoken, synth } = engineOver([HORTENSE]);

    const first = engine.speak([{ text: "Premier.", pauseAfterMs: 0 }], OPTIONS);
    await flush();
    const second = engine.speak([{ text: "Second.", pauseAfterMs: 0 }], OPTIONS);
    await flush();

    expect(synth.cancel).toHaveBeenCalled();
    await expect(first).resolves.toBeUndefined();
    expect(spoken.map((each) => each.text)).toStrictEqual(["Premier.", "Second."]);
    spoken[1]?.onend?.();
    await expect(second).resolves.toBeUndefined();
  });

  it("stop ends the speech at once, cancels the synthesis, drops the rest and is not a failure", async () => {
    const { engine, spoken, synth, clock } = engineOver([HORTENSE]);
    const progress: number[] = [];

    const reading = engine.speak(
      [
        { text: "Un.", pauseAfterMs: 500 },
        { text: "Deux.", pauseAfterMs: 0 },
      ],
      { ...OPTIONS, onProgress: ({ done }) => progress.push(done) },
    );
    await flush();
    engine.stop();

    await expect(reading).resolves.toBeUndefined();
    expect(synth.cancel).toHaveBeenCalledTimes(1);
    clock.fireAll();
    await flush();
    expect(spoken).toHaveLength(1);
    expect(progress).toStrictEqual([0]);
  });

  it("stop during the rest between two segments ends it and clears the timer", async () => {
    const { engine, spoken, clock } = engineOver([HORTENSE]);

    const reading = engine.speak(
      [
        { text: "Un.", pauseAfterMs: 500 },
        { text: "Deux.", pauseAfterMs: 0 },
      ],
      OPTIONS,
    );
    await flush();
    spoken[0]?.onend?.();
    await flush();
    expect(clock.pending.size).toBe(1);

    engine.stop();
    await expect(reading).resolves.toBeUndefined();
    expect(clock.pending.size).toBe(0);
    expect(spoken).toHaveLength(1);
  });

  it("rejects with tts_failed when the system reports an error that is not a cancel", async () => {
    const { engine, spoken } = engineOver([HORTENSE]);

    const reading = engine.speak([{ text: "Un.", pauseAfterMs: 0 }], OPTIONS);
    await flush();
    spoken[0]?.onerror?.({ error: "synthesis-failed" });

    await expect(reading).rejects.toStrictEqual(new SpeechOutputError("tts_failed"));
  });

  it("lists only the local voices of the language", async () => {
    const { engine } = engineOver([HORTENSE, DENISE, ZIRA, CELINE]);

    await expect(engine.voices("fr-FR")).resolves.toStrictEqual([
      { id: "Hortense", name: "Hortense", lang: "fr-FR" },
      { id: "Celine", name: "Celine", lang: "fr-CA" },
    ]);
  });
});

describe("waiting for the voices of the system", () => {
  it("answers at once when the list is already there", async () => {
    const { synth } = fakeSynth([HORTENSE]);

    await expect(loadVoices(synth)).resolves.toStrictEqual([HORTENSE]);
  });

  it("waits for voiceschanged when the list is empty, then stops listening", async () => {
    const world = fakeSynth([]);
    const clock = fakeTimers();

    const loading = loadVoices(world.synth, 1500, clock.timers);
    world.setVoices([HORTENSE]);

    await expect(loading).resolves.toStrictEqual([HORTENSE]);
    expect(world.listenerCount()).toBe(0);
    expect(clock.pending.size).toBe(0);
  });

  it("gives up after the timeout with an empty list", async () => {
    const world = fakeSynth([]);
    const clock = fakeTimers();

    const loading = loadVoices(world.synth, 1500, clock.timers);
    expect([...clock.pending.values()].map((timer) => timer.ms)).toStrictEqual([1500]);
    clock.fireAll();

    await expect(loading).resolves.toStrictEqual([]);
    expect(world.listenerCount()).toBe(0);
  });
});
