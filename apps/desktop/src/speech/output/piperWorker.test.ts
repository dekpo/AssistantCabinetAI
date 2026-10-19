import { describe, expect, it, vi } from "vitest";
import { SpeechOutputError, type SpeakSegment } from "./engine";
import { createPiperEngine, installedVoices, type PiperDeps, type Timers } from "./piperWorker";
import type { BufferPlayer } from "./playback";
import type { SpeechStatus } from "../ipc";

/** What the fake world records, in order, so the look-ahead and the stops can be read back. */
type Event = string;

interface Pending {
  text: string;
  resolve: (wav: Uint8Array) => void;
  reject: (error: unknown) => void;
}

function world() {
  const events: Event[] = [];
  const asked: Pending[] = [];
  const playing: Array<{ wav: Uint8Array; end: () => void; fail: (error: unknown) => void }> = [];
  const timersSet: Array<{ callback: () => void; ms: number; cleared: boolean }> = [];

  const player: BufferPlayer = {
    play: (wav) =>
      new Promise<void>((resolve, reject) => {
        events.push(`play:${wav[0]}`);
        playing.push({ wav, end: resolve, fail: reject });
      }),
    stop: vi.fn(() => {
      events.push("player-stop");
      playing.splice(0).forEach((clip) => clip.end());
    }),
    dispose: async () => {},
  };
  const timers: Timers = {
    set: (callback, ms) => {
      const timer = { callback, ms, cleared: false };
      timersSet.push(timer);
      return timer;
    },
    clear: (handle) => {
      (handle as { cleared: boolean }).cleared = true;
    },
  };
  const deps: PiperDeps = {
    synthesize: (text, locale) =>
      new Promise((resolve, reject) => {
        events.push(`synthesize:${text}:${locale}`);
        asked.push({ text, resolve, reject });
      }),
    cancel: vi.fn(async () => {
      events.push("cancel");
      return true;
    }),
    player,
    voices: async () => [],
    timers,
  };
  return { events, asked, playing, timersSet, deps, player };
}

const segments = (...texts: string[]): SpeakSegment[] =>
  texts.map((text) => ({ text, pauseAfterMs: 380 }));
const wav = (marker: number) => new Uint8Array([marker]);
const flush = async () => {
  for (let index = 0; index < 12; index += 1) {
    await Promise.resolve();
  }
};
const options = { locale: "fr-FR", rate: 1, voiceId: null };

describe("the Piper engine", () => {
  it("speaks the segments in order and asks for the next one while the current one plays", async () => {
    const w = world();
    const progress: number[] = [];
    const engine = createPiperEngine(w.deps);

    const reading = engine.speak(segments("un", "deux", "trois"), {
      ...options,
      onProgress: ({ done }) => progress.push(done),
    });
    await flush();
    expect(w.events).toStrictEqual(["synthesize:un:fr-FR"]);

    w.asked[0]?.resolve(wav(1));
    await flush();
    // The first one plays; the second is already being prepared, the third is not.
    expect(w.events).toStrictEqual(["synthesize:un:fr-FR", "synthesize:deux:fr-FR", "play:1"]);

    w.playing[0]?.end();
    await flush();
    // The pause of the text is a timer; the second sentence was prepared meanwhile.
    expect(w.timersSet).toHaveLength(1);
    expect(w.timersSet[0]?.ms).toBe(380);
    w.asked[1]?.resolve(wav(2));
    w.timersSet[0]?.callback();
    await flush();
    expect(w.events.slice(-2)).toStrictEqual(["synthesize:trois:fr-FR", "play:2"]);

    w.playing[1]?.end();
    await flush();
    w.timersSet[1]?.callback();
    w.asked[2]?.resolve(wav(3));
    await flush();
    w.playing[2]?.end();
    await reading;

    expect(progress).toStrictEqual([0, 1, 2, 3]);
    expect(w.timersSet).toHaveLength(2);
    expect(w.deps.cancel).not.toHaveBeenCalled();
  });

  it("prepares one segment ahead and no more", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);

    void engine.speak(segments("a", "b", "c", "d"), options);
    await flush();
    w.asked[0]?.resolve(wav(1));
    await flush();

    expect(w.asked.map((request) => request.text)).toStrictEqual(["a", "b"]);
  });

  it("stop ends the playing clip, cancels the synthesis and resolves without an error", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un", "deux"), options);
    await flush();
    w.asked[0]?.resolve(wav(1));
    await flush();

    engine.stop();

    await expect(reading).resolves.toBeUndefined();
    expect(w.player.stop).toHaveBeenCalledTimes(1);
    expect(w.deps.cancel).toHaveBeenCalledTimes(1);
    // The sentence prepared ahead finishes after the stop and is never played.
    w.asked[1]?.resolve(wav(2));
    await flush();
    expect(w.events.filter((event) => event.startsWith("play:"))).toStrictEqual(["play:1"]);
  });

  it("stop while the first sentence is still being prepared plays nothing", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un"), options);
    await flush();

    engine.stop();
    w.asked[0]?.resolve(wav(1));

    await expect(reading).resolves.toBeUndefined();
    expect(w.events.filter((event) => event.startsWith("play:"))).toHaveLength(0);
  });

  it("stop during a pause ends the pause", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un", "deux"), options);
    await flush();
    w.asked[0]?.resolve(wav(1));
    await flush();
    w.playing[0]?.end();
    await flush();
    expect(w.timersSet).toHaveLength(1);

    engine.stop();

    await expect(reading).resolves.toBeUndefined();
    expect(w.timersSet[0]?.cleared).toBe(true);
  });

  it("starting a new reading ends the one in progress", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const first = engine.speak(segments("old"), options);
    await flush();

    const second = engine.speak(segments("new"), options);
    await flush();

    await expect(first).resolves.toBeUndefined();
    expect(w.deps.cancel).toHaveBeenCalledTimes(1);
    w.asked[1]?.resolve(wav(9));
    await flush();
    w.playing[0]?.end();
    await expect(second).resolves.toBeUndefined();
  });

  it("a code from Rust is kept, so it has its own sentence", async () => {
    for (const code of ["tts_no_local_voice", "speech_model_corrupt", "speech_busy", "tts_failed"]) {
      const w = world();
      const engine = createPiperEngine(w.deps);
      const reading = engine.speak(segments("un"), options);
      await flush();

      w.asked[0]?.reject({ code, message: "x" });

      await expect(reading).rejects.toStrictEqual(new SpeechOutputError(code));
    }
  });

  it("an error without a code is a failed reading, and its message never travels", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un"), options);
    await flush();

    w.asked[0]?.reject(new Error("a technical English message"));

    await expect(reading).rejects.toStrictEqual(new SpeechOutputError("tts_failed"));
  });

  it("a synthesis that Rust cancelled is a stop, not a failure", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un"), options);
    await flush();

    w.asked[0]?.reject({ code: "speech_cancelled" });

    await expect(reading).resolves.toBeUndefined();
  });

  it("a failure of the sentence prepared ahead surfaces when its turn comes, and stops the rest", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un", "deux", "trois"), options);
    await flush();
    w.asked[0]?.resolve(wav(1));
    await flush();
    // It fails while the first one is still playing: no unhandled rejection, nothing yet.
    w.asked[1]?.reject({ code: "tts_failed" });
    await flush();
    w.playing[0]?.end();
    await flush();
    // The text's own pause comes first; the failure surfaces when the second sentence is due.
    w.timersSet[0]?.callback();
    await flush();

    await expect(reading).rejects.toStrictEqual(new SpeechOutputError("tts_failed"));
    expect(w.asked).toHaveLength(2);
    expect(w.deps.cancel).toHaveBeenCalled();
  });

  it("a clip that cannot be played is a failed reading", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);
    const reading = engine.speak(segments("un"), options);
    await flush();
    w.asked[0]?.resolve(wav(1));
    await flush();

    w.playing[0]?.fail(new Error("decode"));

    await expect(reading).rejects.toStrictEqual(new SpeechOutputError("tts_failed"));
  });

  it("sends the text and the language and nothing else: the voice and the speed are Rust's", async () => {
    const w = world();
    const synthesize = vi.fn(w.deps.synthesize);
    const engine = createPiperEngine({ ...w.deps, synthesize });
    void engine.speak(segments("un"), { locale: "en-US", rate: 1.7, voiceId: "libritts_r" });
    await flush();

    expect(synthesize.mock.calls).toStrictEqual([["un", "en-US"]]);
  });

  it("an empty list of segments speaks nothing and asks for nothing", async () => {
    const w = world();
    const engine = createPiperEngine(w.deps);

    await expect(engine.speak([], options)).resolves.toBeUndefined();

    expect(w.events).toStrictEqual([]);
  });

  it("stop with nothing playing does nothing", () => {
    const w = world();
    createPiperEngine(w.deps).stop();

    expect(w.player.stop).not.toHaveBeenCalled();
    expect(w.deps.cancel).not.toHaveBeenCalled();
  });
});

describe("the voices the engine would use", () => {
  const status = (voices: SpeechStatus["voices"]): SpeechStatus => ({
    enabled: true,
    locale: "fr-FR",
    features: {
      dictation: { available: true, reason: null },
      readAloud: { available: true, reason: null },
      notes: { available: true, reason: null },
    },
    sttModel: "present",
    languages: [
      {
        id: "fr-FR",
        support: "enabled",
        dictation: true,
        readAloud: true,
        voices: [
          { gender: "female", id: "siwis" },
          { gender: "male", id: "css10" },
        ],
      },
    ],
    voices,
  });

  it("lists the installed voices of the language, in the order of the language table", () => {
    const listed = installedVoices(
      status([
        { id: "css10", state: "present", rating: "attribution", bytes: 1 },
        { id: "siwis", state: "unverified", rating: "review", bytes: 1 },
        { id: "ljspeech", state: "present", rating: "clear", bytes: 1 },
      ]),
      "fr-FR",
    );

    expect(listed.map((voice) => voice.id)).toStrictEqual(["css10"]);
  });

  it("is empty for a language the table does not know", () => {
    expect(installedVoices(status([]), "de-DE")).toStrictEqual([]);
  });
});
