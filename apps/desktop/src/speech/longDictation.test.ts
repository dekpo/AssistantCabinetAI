import { describe, expect, it, vi } from "vitest";
import { CUT_SILENCE_SECONDS } from "../audio/segmenter";
import { CaptureError, type AudioCapture, type CaptureResult } from "../audio/recorder";
import { CAP_MARGIN_MS } from "./dictation";
import type { AppliedCommands, SpeechTranscript } from "./ipc";
import {
  createLongDictation,
  escapeCancelsLong,
  isActive,
  type Delivery,
  type LongDictation,
  type LongDictationDeps,
} from "./longDictation";
import { createVoiceSession, type VoiceSession } from "./voiceSession";

const RATE = 16_000;
const BLOCK = 1_024;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const heard = (text: string, caseReliable = true): SpeechTranscript => ({
  text,
  audioMs: 3000,
  processingMs: 400,
  engineId: "fake",
  caseReliable,
});

/** A recorder the test drives by hand: it settles `start`, and the test pushes the blocks. */
function fakeCapture() {
  const starting = deferred<void>();
  let sink: ((block: Float32Array, rate: number) => void) | undefined;
  let levelSink: ((level: number) => void) | undefined;
  const capture: AudioCapture = {
    start: vi.fn((options) => {
      sink = options?.onBlock;
      levelSink = options?.onLevel;
      return starting.promise;
    }),
    stop: vi.fn(() =>
      Promise.resolve({
        wav: new Uint8Array(),
        durationSeconds: 0,
        stats: { samples: 0, peak: 0, clippedShare: 0 },
        resampled: false,
      }),
    ),
    cancel: vi.fn(),
  };
  const push = (samples: Float32Array) => {
    for (let from = 0; from < samples.length; from += BLOCK) {
      sink?.(samples.slice(from, Math.min(samples.length, from + BLOCK)), RATE);
    }
  };
  return {
    capture,
    starting,
    level: (value: number) => levelSink?.(value),
    /** Speech of `seconds`, then a pause of `pause` seconds: one passage once the pause is long enough. */
    say: (seconds: number, pause = CUT_SILENCE_SECONDS + 0.3) => {
      const speech = new Float32Array(Math.round(seconds * RATE));
      for (let index = 0; index < speech.length; index += 1) {
        speech[index] = 0.2 * Math.sin((index / RATE) * 2 * Math.PI * 220);
      }
      push(speech);
      push(new Float32Array(Math.round(pause * RATE)));
    },
  };
}

interface Setup {
  dictation: LongDictation;
  recorders: ReturnType<typeof fakeCapture>[];
  transcribe: ReturnType<typeof vi.fn>;
  /** The engine calls still waiting for the test to answer, oldest first. */
  engine: ReturnType<typeof deferred<SpeechTranscript>>[];
  delivered: Delivery[];
  cancelEngine: ReturnType<typeof vi.fn>;
  applyCommands: ReturnType<typeof vi.fn>;
  session: VoiceSession;
  timers: { run: () => void; ms: number; cancelled: boolean }[];
  flags: { ready: boolean; blocked: boolean };
}

function setup(overrides: Partial<LongDictationDeps> = {}): Setup {
  const recorders: ReturnType<typeof fakeCapture>[] = [];
  const engine: Setup["engine"] = [];
  const delivered: Delivery[] = [];
  const transcribe = vi.fn((_wav: Uint8Array) => {
    const answer = deferred<SpeechTranscript>();
    engine.push(answer);
    return answer.promise;
  });
  const cancelEngine = vi.fn();
  const applyCommands = vi.fn(
    (text: string, _pending: number): Promise<AppliedCommands> =>
      Promise.resolve({ pieces: [text], pendingHeading: 0 }),
  );
  const session = createVoiceSession();
  const timers: Setup["timers"] = [];
  const flags = { ready: true, blocked: false };
  let clips = 0;
  const dictation = createLongDictation({
    capture: () => {
      const recorder = fakeCapture();
      recorders.push(recorder);
      return recorder.capture;
    },
    clip: (samples): Promise<CaptureResult> => {
      clips += 1;
      return Promise.resolve({
        wav: new Uint8Array([clips]),
        durationSeconds: samples.length / RATE,
        stats: { samples: samples.length, peak: 0.4, clippedShare: 0 },
        resampled: false,
      });
    },
    transcribe,
    cancelEngine,
    applyCommands,
    deliver: (delivery) => delivered.push(delivery),
    session,
    schedule: (run, ms) => {
      const timer = { run, ms, cancelled: false };
      timers.push(timer);
      return () => {
        timer.cancelled = true;
      };
    },
    now: () => 1_000,
    maxSeconds: () => 1_800,
    ready: () => flags.ready,
    blocked: () => flags.blocked,
    ...overrides,
  });
  return { dictation, recorders, transcribe, engine, delivered, cancelEngine, applyCommands, session, timers, flags };
}

/** Starts a recording and lets the microphone open. */
async function recording(env: Setup) {
  env.dictation.start();
  const recorder = env.recorders[env.recorders.length - 1] as ReturnType<typeof fakeCapture>;
  recorder.starting.resolve();
  await flush();
  return recorder;
}

describe("starting", () => {
  it("claims the microphone, asks for it, then records with a clock", async () => {
    const env = setup();
    env.dictation.start();

    expect(env.dictation.getSnapshot().phase).toBe("requesting-permission");
    expect(env.session.holder()).toBe("notes");

    (env.recorders[0] as ReturnType<typeof fakeCapture>).starting.resolve();
    await flush();

    expect(env.dictation.getSnapshot().phase).toBe("recording");
    expect(env.dictation.getSnapshot().startedAt).toBe(1_000);
    expect(isActive("recording")).toBe(true);
    expect(isActive("idle")).toBe(false);
  });

  it("does nothing when the voice is not ready, or the notes are blocked", () => {
    const env = setup();
    env.flags.ready = false;
    env.dictation.start();
    env.flags.ready = true;
    env.flags.blocked = true;
    env.dictation.start();

    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(env.recorders).toHaveLength(0);
  });

  it("is refused while the question box is dictating", () => {
    const env = setup();
    env.session.claim("dictation", () => {});

    env.dictation.start();

    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(env.dictation.getSnapshot().error?.key).toBe("speech.session.microphoneBusy");
    expect(env.recorders).toHaveLength(0);
  });

  it("reports a refused microphone with its sentence and gives the session back", async () => {
    const env = setup();
    env.dictation.start();
    (env.recorders[0] as ReturnType<typeof fakeCapture>).starting.reject(new CaptureError("denied"));
    await flush();

    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(env.dictation.getSnapshot().error?.key).toBe("speech.microphone.denied");
    expect(env.session.holder()).toBeNull();
  });

  it("starts once: a second press while recording starts nothing", async () => {
    const env = setup();
    await recording(env);

    env.dictation.start();

    expect(env.recorders).toHaveLength(1);
  });

  it("stops at the cap, a little before the length the engine accepts", async () => {
    const env = setup();
    await recording(env);

    expect(env.timers[0]?.ms).toBe(1_800 * 1_000 - CAP_MARGIN_MS);
    env.timers[0]?.run();
    await flush();

    expect(env.dictation.getSnapshot().capReached).toBe(true);
    expect(env.dictation.getSnapshot().phase).toBe("idle");
  });
});

describe("the passages", () => {
  it("transcribes each passage as it is cut, while the user goes on", async () => {
    const env = setup();
    const recorder = await recording(env);

    recorder.say(3);
    await flush();

    expect(env.dictation.getSnapshot().phase).toBe("recording");
    expect(env.transcribe).toHaveBeenCalledTimes(1);
    expect(env.dictation.getSnapshot().waiting).toBe(1);
  });

  it("runs one job at a time, in the order the passages were cut", async () => {
    const env = setup();
    const recorder = await recording(env);

    recorder.say(3);
    recorder.say(3);
    recorder.say(3);
    await flush();

    expect(env.transcribe).toHaveBeenCalledTimes(1);
    expect(env.dictation.getSnapshot().waiting).toBe(3);

    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("un"));
    await flush();
    expect(env.transcribe).toHaveBeenCalledTimes(2);
    expect(env.dictation.getSnapshot().waiting).toBe(2);

    (env.engine[1] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("deux"));
    await flush();
    (env.engine[2] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("trois"));
    await flush();

    expect(env.delivered.map((entry) => entry.pieces[0])).toStrictEqual(["un", "deux", "trois"]);
    expect(env.transcribe.mock.calls.map(([wav]) => (wav as Uint8Array)[0])).toStrictEqual([1, 2, 3]);
  });

  it("counts the backlog down to nothing", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3);
    recorder.say(3);
    await flush();
    const waiting: number[] = [env.dictation.getSnapshot().waiting];
    env.dictation.subscribe(() => waiting.push(env.dictation.getSnapshot().waiting));

    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("un"));
    await flush();
    (env.engine[1] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("deux"));
    await flush();

    expect(waiting[0]).toBe(2);
    expect(waiting[waiting.length - 1]).toBe(0);
    expect(waiting).toContain(1);
  });

  it("hands the text through the voice commands, with the heading left open", async () => {
    const env = setup();
    env.applyCommands
      .mockResolvedValueOnce({ pieces: [""], pendingHeading: 1 })
      .mockResolvedValueOnce({ pieces: ["# Titre"], pendingHeading: 0 });
    const recorder = await recording(env);

    recorder.say(3);
    await flush();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("Titre."));
    await flush();
    recorder.say(3);
    await flush();
    (env.engine[1] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("Visite."));
    await flush();

    expect(env.applyCommands.mock.calls).toStrictEqual([
      ["Titre.", 0],
      ["Visite.", 1],
    ]);
  });

  it("delivers a new note's pieces as they come back, with the engine's capital letters flag", async () => {
    const env = setup();
    env.applyCommands.mockResolvedValueOnce({ pieces: ["avant", "apres"], pendingHeading: 0 });
    const recorder = await recording(env);

    recorder.say(3);
    await flush();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("avant nouvelle note apres", false));
    await flush();

    expect(env.delivered).toStrictEqual([{ pieces: ["avant", "apres"], caseReliable: false }]);
  });

  it("delivers the raw text when the commands cannot be applied", async () => {
    const env = setup();
    env.applyCommands.mockRejectedValueOnce({ code: "speech_locale_unsupported", data: {} });
    const recorder = await recording(env);

    recorder.say(3);
    await flush();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("bonjour"));
    await flush();

    expect(env.delivered).toStrictEqual([{ pieces: ["bonjour"], caseReliable: true }]);
  });

  it("delivers nothing for an empty transcript", async () => {
    const env = setup();
    const recorder = await recording(env);

    recorder.say(3);
    await flush();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("   "));
    await flush();

    expect(env.delivered).toHaveLength(0);
    expect(env.applyCommands).not.toHaveBeenCalled();
  });

  it("sends nothing to the engine for a recording with no speech in it", async () => {
    const env = setup();
    const recorder = await recording(env);

    recorder.say(0.01, 20);
    env.dictation.stop();
    await flush();

    expect(env.transcribe).not.toHaveBeenCalled();
    expect(env.dictation.getSnapshot().phase).toBe("idle");
  });

  it("keeps the level on its own channel", async () => {
    const env = setup();
    const recorder = await recording(env);
    const seen: number[] = [];
    env.dictation.subscribeLevel(() => seen.push(env.dictation.getLevel()));

    recorder.level(0.4);
    env.dictation.cancel();

    expect(seen).toStrictEqual([0.4, 0]);
  });
});

describe("stopping", () => {
  it("finishes the queue: the last passage is cut, transcribed and put in the note", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3, 0.1);
    await flush();

    env.dictation.stop();
    await flush();

    expect(env.dictation.getSnapshot().phase).toBe("finishing");
    expect(env.dictation.getSnapshot().startedAt).toBeNull();
    expect(env.session.holder()).toBe("notes");

    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("fin"));
    await flush();

    expect(env.delivered).toHaveLength(1);
    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(env.dictation.getSnapshot().waiting).toBe(0);
    expect(env.session.holder()).toBeNull();
  });

  it("is idle at once when nothing is waiting", async () => {
    const env = setup();
    await recording(env);

    env.dictation.stop();
    await flush();

    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(env.session.holder()).toBeNull();
  });

  it("ignores a stop when nothing is recording", () => {
    const env = setup();

    env.dictation.stop();

    expect(env.dictation.getSnapshot().phase).toBe("idle");
  });
});

describe("cancelling", () => {
  it("drops the queue, stops the engine and ignores its late answer", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3);
    recorder.say(3);
    await flush();

    env.dictation.cancel();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("trop tard"));
    await flush();

    expect(env.cancelEngine).toHaveBeenCalledTimes(1);
    expect(env.transcribe).toHaveBeenCalledTimes(1);
    expect(env.delivered).toHaveLength(0);
    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(env.dictation.getSnapshot().waiting).toBe(0);
    expect(env.session.holder()).toBeNull();
    expect((env.recorders[0] as ReturnType<typeof fakeCapture>).capture.cancel).toHaveBeenCalled();
  });

  it("works at the permission stage, and the late grant is let go", async () => {
    const env = setup();
    env.dictation.start();
    const recorder = env.recorders[0] as ReturnType<typeof fakeCapture>;

    env.dictation.cancel();
    recorder.starting.resolve();
    await flush();

    expect(env.dictation.getSnapshot().phase).toBe("idle");
    expect(recorder.capture.cancel).toHaveBeenCalled();
    expect(env.cancelEngine).not.toHaveBeenCalled();
  });

  it("works while the queue is being finished", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3, 0.1);
    await flush();
    env.dictation.stop();
    await flush();

    env.dictation.cancel();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("fin"));
    await flush();

    expect(env.delivered).toHaveLength(0);
    expect(env.dictation.getSnapshot().phase).toBe("idle");
  });

  it("lets a new recording start straight after, with its own count", async () => {
    const env = setup();
    const first = await recording(env);
    first.say(3);
    await flush();
    env.dictation.cancel();

    const second = await recording(env);
    second.say(3);
    await flush();
    (env.engine[1] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("neuf"));
    await flush();

    expect(env.delivered).toStrictEqual([{ pieces: ["neuf"], caseReliable: true }]);
  });

  it("is a no-op at rest", () => {
    const env = setup();
    const changes = vi.fn();
    env.dictation.subscribe(changes);

    env.dictation.cancel();

    expect(changes).not.toHaveBeenCalled();
  });
});

describe("a passage that fails", () => {
  it("is reported by its position and the others go on", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3);
    recorder.say(3);
    recorder.say(3);
    await flush();

    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("un"));
    await flush();
    (env.engine[1] as ReturnType<typeof deferred<SpeechTranscript>>).reject({
      code: "speech_engine_failed",
      data: {},
    });
    await flush();
    (env.engine[2] as ReturnType<typeof deferred<SpeechTranscript>>).resolve(heard("trois"));
    await flush();
    env.dictation.stop();
    await flush();

    const snapshot = env.dictation.getSnapshot();
    expect(snapshot.failed).toStrictEqual([2]);
    expect(snapshot.error?.key).toBe("errors.speech_engine_failed");
    expect(env.delivered.map((entry) => entry.pieces[0])).toStrictEqual(["un", "trois"]);
    expect(snapshot.phase).toBe("idle");
  });

  it("is not a failure when the engine finds the passage too short or too quiet", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3);
    await flush();

    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).reject({
      code: "speech_audio_too_quiet",
      data: {},
    });
    await flush();

    expect(env.dictation.getSnapshot().failed).toStrictEqual([]);
    expect(env.dictation.getSnapshot().error).toBeNull();
  });

  it("keeps its notice after the recording, until it is dismissed", async () => {
    const env = setup();
    const recorder = await recording(env);
    recorder.say(3);
    await flush();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).reject({
      code: "speech_timeout",
      data: {},
    });
    await flush();
    env.dictation.stop();
    await flush();

    expect(env.dictation.getSnapshot().failed).toStrictEqual([1]);

    env.dictation.dismiss();

    expect(env.dictation.getSnapshot().failed).toStrictEqual([]);
    expect(env.dictation.getSnapshot().error).toBeNull();
  });

  it("is forgotten by the next recording", async () => {
    const env = setup();
    const first = await recording(env);
    first.say(3);
    await flush();
    (env.engine[0] as ReturnType<typeof deferred<SpeechTranscript>>).reject({ code: "speech_timeout", data: {} });
    await flush();
    env.dictation.stop();
    await flush();

    await recording(env);

    expect(env.dictation.getSnapshot().failed).toStrictEqual([]);
    expect(env.dictation.getSnapshot().error).toBeNull();
  });
});

describe("the Escape key", () => {
  it("ends a recording or its queue, but not under a dialog or a key already used", () => {
    expect(escapeCancelsLong("Escape", "recording", false, false)).toBe(true);
    expect(escapeCancelsLong("Escape", "finishing", false, false)).toBe(true);
    expect(escapeCancelsLong("Escape", "idle", false, false)).toBe(false);
    expect(escapeCancelsLong("Escape", "recording", true, false)).toBe(false);
    expect(escapeCancelsLong("Escape", "recording", false, true)).toBe(false);
    expect(escapeCancelsLong("Enter", "recording", false, false)).toBe(false);
  });
});
