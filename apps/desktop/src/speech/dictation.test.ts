import { describe, expect, it, vi } from "vitest";
import { CaptureError, type AudioCapture, type CaptureResult } from "../audio/recorder";
import {
  CAP_MARGIN_MS,
  createDictation,
  escapeCancelsDictation,
  MIN_DICTATION_SECONDS,
  shownPhase,
  type Dictation,
  type DictationDeps,
} from "./dictation";
import type { SpeechTranscript } from "./ipc";
import { createVoiceSession, type VoiceSession } from "./voiceSession";

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

function clipOf(overrides: Partial<CaptureResult> = {}, peak = 0.4, clippedShare = 0): CaptureResult {
  return {
    wav: new Uint8Array([1, 2, 3]),
    durationSeconds: 3,
    stats: { samples: 48_000, peak, clippedShare },
    resampled: false,
    ...overrides,
  };
}

const heard = (text: string): SpeechTranscript => ({
  text,
  audioMs: 3000,
  processingMs: 400,
  engineId: "fake",
  caseReliable: true,
});

/** A recorder the test drives by hand: it settles `start` and `stop` when told to. */
function fakeCapture() {
  const starting = deferred<void>();
  const stopping = deferred<CaptureResult>();
  let levelSink: ((level: number) => void) | undefined;
  const capture: AudioCapture = {
    start: vi.fn((options) => {
      levelSink = options?.onLevel;
      return starting.promise;
    }),
    stop: vi.fn(() => stopping.promise),
    cancel: vi.fn(),
  };
  return { capture, starting, stopping, level: (value: number) => levelSink?.(value) };
}

interface Setup {
  dictation: Dictation;
  recorders: ReturnType<typeof fakeCapture>[];
  draft: { value: string };
  transcribe: ReturnType<typeof vi.fn>;
  cancelEngine: ReturnType<typeof vi.fn>;
  session: VoiceSession;
  timers: { run: () => void; ms: number; cancelled: boolean }[];
  flags: { ready: boolean; blocked: boolean };
}

function setup(overrides: Partial<DictationDeps> = {}, startDraft = ""): Setup {
  const recorders: ReturnType<typeof fakeCapture>[] = [];
  const draft = { value: startDraft };
  const transcribe = vi.fn((_wav: Uint8Array) => Promise.resolve(heard("bonjour docteur")));
  const cancelEngine = vi.fn();
  const session = createVoiceSession();
  const timers: Setup["timers"] = [];
  const flags = { ready: true, blocked: false };
  const dictation = createDictation({
    capture: () => {
      const recorder = fakeCapture();
      recorders.push(recorder);
      return recorder.capture;
    },
    transcribe,
    cancelEngine,
    session,
    schedule: (run, ms) => {
      const timer = { run, ms, cancelled: false };
      timers.push(timer);
      return () => {
        timer.cancelled = true;
      };
    },
    maxSeconds: () => 120,
    ready: () => flags.ready,
    blocked: () => flags.blocked,
    getDraft: () => draft.value,
    setDraft: (text) => {
      draft.value = text;
    },
    ...overrides,
  });
  return { dictation, recorders, draft, transcribe, cancelEngine, session, timers, flags };
}

const caretAtStart = { start: 0, end: 0 };

/** Starts and lets the microphone open: the dictation is recording. */
async function recording(world: Setup, selection = caretAtStart) {
  world.dictation.start(selection);
  world.recorders[0]?.starting.resolve();
  await flush();
}

/** Presses stop and hands back the clip: the engine has been called (or the clip refused). */
async function finished(world: Setup, clip: CaptureResult = clipOf()) {
  world.dictation.stop();
  world.recorders[0]?.stopping.resolve(clip);
  await flush();
}

describe("the states of a dictation", () => {
  it("goes idle, requesting-permission, recording, transcribing, done", async () => {
    const world = setup();
    const seen = [world.dictation.getSnapshot().phase];
    world.dictation.subscribe(() => seen.push(world.dictation.getSnapshot().phase));

    world.dictation.start(caretAtStart);
    expect(world.dictation.getSnapshot().phase).toBe("requesting-permission");
    expect(world.session.holder()).toBe("dictation");

    world.recorders[0]?.starting.resolve();
    await flush();
    expect(world.dictation.getSnapshot().phase).toBe("recording");

    world.dictation.stop();
    expect(world.dictation.getSnapshot().phase).toBe("transcribing");
    world.recorders[0]?.stopping.resolve(clipOf());
    await flush();

    expect(seen).toEqual(["idle", "requesting-permission", "recording", "transcribing", "done"]);
    expect(world.draft.value).toBe("bonjour docteur");
    expect(world.session.holder()).toBeNull();
    expect(world.transcribe).toHaveBeenCalledTimes(1);
  });

  it("hands the engine the clip and nothing else", async () => {
    const world = setup();
    await recording(world);
    await finished(world);

    expect(world.transcribe).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]));
  });

  it("puts the words at the remembered selection and reports the numbers and the clipping", async () => {
    const world = setup(
      { transcribe: () => Promise.resolve(heard("Marie Martin, 3 fois le 12")) },
      "Voir XXX demain",
    );
    await recording(world, { start: 5, end: 8 });
    await finished(world, clipOf({}, 1, 0.02));

    const snapshot = world.dictation.getSnapshot();
    expect(world.draft.value).toBe("Voir Marie Martin, 3 fois le 12 demain");
    expect(snapshot.result?.numbers).toBe(2);
    expect(snapshot.result?.clipped).toBe(true);
    expect(snapshot.result?.engineId).toBe("fake");
    expect(snapshot.result?.caseReliable).toBe(true);
    expect(snapshot.caret?.position).toBe("Voir Marie Martin, 3 fois le 12".length);
    expect(snapshot.caret?.text).toBe(world.draft.value);
  });

  it("appends when the draft changed while she was speaking", async () => {
    const world = setup({}, "Bonjour");
    await recording(world, { start: 0, end: 7 });
    world.draft.value = "Bonjour a tous";
    await finished(world);

    expect(world.draft.value).toBe("Bonjour a tous bonjour docteur");
  });

  it("starts again from done and from error", async () => {
    const world = setup();
    await recording(world);
    await finished(world);
    expect(world.dictation.getSnapshot().phase).toBe("done");

    world.dictation.start(caretAtStart);
    expect(world.dictation.getSnapshot().phase).toBe("requesting-permission");
    world.recorders[1]?.starting.reject(new CaptureError("denied"));
    await flush();
    expect(world.dictation.getSnapshot().phase).toBe("error");

    world.dictation.start(caretAtStart);
    expect(world.dictation.getSnapshot().phase).toBe("requesting-permission");
  });

  it("takes one recording at a time", async () => {
    const world = setup();
    await recording(world);

    world.dictation.start(caretAtStart);

    expect(world.recorders).toHaveLength(1);
  });

  it("does not start when the engine is not there or the box is not free", () => {
    const world = setup();
    world.flags.ready = false;
    world.dictation.start(caretAtStart);
    world.flags.ready = true;
    world.flags.blocked = true;
    world.dictation.start(caretAtStart);

    expect(world.recorders).toHaveLength(0);
    expect(world.session.holder()).toBeNull();
    expect(world.dictation.getSnapshot().phase).toBe("idle");
  });

  it("shows unavailable only when nothing is going on", () => {
    expect(shownPhase("idle", false)).toBe("unavailable");
    expect(shownPhase("done", false)).toBe("unavailable");
    expect(shownPhase("error", false)).toBe("unavailable");
    expect(shownPhase("recording", false)).toBe("recording");
    expect(shownPhase("transcribing", false)).toBe("transcribing");
    expect(shownPhase("idle", true)).toBe("idle");
  });
});

describe("cancelling a dictation, at every stage", () => {
  it("while the microphone opens: the late answer is dropped and the recorder is released", async () => {
    const world = setup();
    world.dictation.start(caretAtStart);

    world.dictation.cancel();
    world.recorders[0]?.starting.resolve();
    await flush();

    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.recorders[0]?.capture.cancel).toHaveBeenCalled();
    expect(world.session.holder()).toBeNull();
    expect(world.timers).toHaveLength(0);
    expect(world.transcribe).not.toHaveBeenCalled();
  });

  it("while recording: nothing is kept and nothing is sent to the engine", async () => {
    const world = setup();
    await recording(world);

    world.dictation.cancel();

    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.recorders[0]?.capture.cancel).toHaveBeenCalled();
    expect(world.timers[0]?.cancelled).toBe(true);
    expect(world.session.holder()).toBeNull();
    expect(world.transcribe).not.toHaveBeenCalled();
    expect(world.cancelEngine).not.toHaveBeenCalled();
  });

  it("while the clip is being closed: the engine is never called", async () => {
    const world = setup();
    await recording(world);
    world.dictation.stop();

    world.dictation.cancel();
    world.recorders[0]?.stopping.resolve(clipOf());
    await flush();

    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.transcribe).not.toHaveBeenCalled();
  });

  it("while transcribing: the engine is stopped and its late answer never reaches the box", async () => {
    const answer = deferred<SpeechTranscript>();
    const world = setup({ transcribe: () => answer.promise }, "Bonjour");
    await recording(world, { start: 7, end: 7 });
    await finished(world);
    expect(world.dictation.getSnapshot().phase).toBe("transcribing");

    world.dictation.cancel();
    answer.resolve(heard("trop tard"));
    await flush();

    expect(world.cancelEngine).toHaveBeenCalledTimes(1);
    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.draft.value).toBe("Bonjour");
    expect(world.session.holder()).toBeNull();
  });

  it("an engine that answers that it was cancelled leaves the box quiet, with no error", async () => {
    const answer = deferred<SpeechTranscript>();
    const world = setup({ transcribe: () => answer.promise });
    await recording(world);
    await finished(world);

    answer.reject({ code: "speech_cancelled", data: {} });
    await flush();

    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.dictation.getSnapshot().error).toBeNull();
  });

  it("from done it only clears the line and keeps the text", async () => {
    const world = setup();
    await recording(world);
    await finished(world);

    world.dictation.cancel();

    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.cancelEngine).not.toHaveBeenCalled();
    expect(world.draft.value).toBe("bonjour docteur");
  });

  it("is safe when nothing is going on, and twice", () => {
    const world = setup();

    world.dictation.cancel();
    world.dictation.cancel();

    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.cancelEngine).not.toHaveBeenCalled();
  });

  it("the cap ends the recording by itself, a margin before the limit Rust enforces", async () => {
    const world = setup();
    await recording(world);

    expect(world.timers).toHaveLength(1);
    expect(world.timers[0]?.ms).toBe(120_000 - CAP_MARGIN_MS);
    world.timers[0]?.run();
    expect(world.dictation.getSnapshot().phase).toBe("transcribing");
  });
});

describe("a clip is refused before any engine call", () => {
  it("under 0.4 s", async () => {
    const world = setup();
    await recording(world);
    await finished(world, clipOf({ durationSeconds: MIN_DICTATION_SECONDS - 0.05 }));

    expect(world.dictation.getSnapshot().phase).toBe("error");
    expect(world.dictation.getSnapshot().error?.key).toBe("errors.speech_audio_too_short");
    expect(world.transcribe).not.toHaveBeenCalled();
    expect(world.session.holder()).toBeNull();
  });

  it("at exactly 0.4 s it goes through", async () => {
    const world = setup();
    await recording(world);
    await finished(world, clipOf({ durationSeconds: MIN_DICTATION_SECONDS }));

    expect(world.transcribe).toHaveBeenCalledTimes(1);
  });

  it("too quiet", async () => {
    const world = setup();
    await recording(world);
    await finished(world, clipOf({}, 0.004));

    expect(world.dictation.getSnapshot().error?.key).toBe("errors.speech_audio_too_quiet");
    expect(world.transcribe).not.toHaveBeenCalled();
  });

  it("an engine answer with no words is an error and leaves the box as it was", async () => {
    const world = setup({ transcribe: () => Promise.resolve(heard("   ")) }, "Bonjour");
    await recording(world);
    await finished(world);

    expect(world.dictation.getSnapshot().error?.key).toBe("speech.dictation.nothingHeard");
    expect(world.draft.value).toBe("Bonjour");
  });
});

describe("what goes wrong is said with a catalogue sentence", () => {
  it.each([
    [new CaptureError("denied"), "speech.microphone.denied"],
    [new CaptureError("missing"), "speech.microphone.missing"],
    [new CaptureError("unsupported"), "speech.microphone.unsupported"],
    [new CaptureError("failed"), "speech.microphone.failed"],
    [{ name: "NotAllowedError" }, "speech.microphone.denied"],
    [{ name: "NotFoundError" }, "speech.microphone.missing"],
    [new Error("anything else"), "speech.microphone.failed"],
  ])("a microphone that does not open (%#)", async (failure, key) => {
    const world = setup();
    world.dictation.start(caretAtStart);

    world.recorders[0]?.starting.reject(failure);
    await flush();

    expect(world.dictation.getSnapshot().phase).toBe("error");
    expect(world.dictation.getSnapshot().error?.key).toBe(key);
    expect(world.session.holder()).toBeNull();
    expect(world.recorders[0]?.capture.cancel).toHaveBeenCalled();
  });

  it("a clip that cannot be closed", async () => {
    const world = setup();
    await recording(world);
    world.dictation.stop();

    world.recorders[0]?.stopping.reject(new CaptureError("failed"));
    await flush();

    expect(world.dictation.getSnapshot().error?.key).toBe("speech.microphone.failed");
  });

  it.each([
    ["speech_busy", "errors.speech_busy"],
    ["speech_engine_missing", "errors.speech_engine_missing"],
    ["speech_model_missing", "errors.speech_model_missing"],
    ["speech_timeout", "errors.speech_timeout"],
    ["speech_output_rejected", "errors.speech_output_rejected"],
  ])("the engine answers %s", async (code, key) => {
    const world = setup({ transcribe: () => Promise.reject({ code, data: {} }) });
    await recording(world);
    await finished(world);

    expect(world.dictation.getSnapshot().error?.key).toBe(key);
    expect(world.draft.value).toBe("");
    expect(world.session.holder()).toBeNull();
  });

  it("an answer that is not a code never shows a raw message", async () => {
    const world = setup({ transcribe: () => Promise.reject(new Error("raw english text")) });
    await recording(world);
    await finished(world);

    expect(world.dictation.getSnapshot().error?.key).toBe("errors.unknown_error");
  });
});

describe("the voice session", () => {
  it("refuses a recording while the notes hold the microphone, without touching the microphone", () => {
    const world = setup();
    world.session.claim("notes", () => undefined);

    world.dictation.start(caretAtStart);

    expect(world.dictation.getSnapshot().error?.key).toBe("speech.session.microphoneBusy");
    expect(world.recorders).toHaveLength(0);
    expect(world.session.holder()).toBe("notes");
  });

  it("stops a reading that is going on and takes the microphone", () => {
    const world = setup();
    const stopReading = vi.fn(() => world.session.release("readAloud"));
    world.session.claim("readAloud", stopReading);

    world.dictation.start(caretAtStart);

    expect(stopReading).toHaveBeenCalledTimes(1);
    expect(world.session.holder()).toBe("dictation");
  });
});

describe("the input level", () => {
  it("follows the microphone while recording and falls back to zero", async () => {
    const world = setup();
    const heardLevels: number[] = [];
    world.dictation.subscribeLevel(() => heardLevels.push(world.dictation.getLevel()));
    await recording(world);

    world.recorders[0]?.level(0.2);
    expect(world.dictation.getLevel()).toBe(0.2);
    world.dictation.cancel();

    expect(world.dictation.getLevel()).toBe(0);
    expect(heardLevels).toEqual([0.2, 0]);
  });

  it("ignores a level that arrives after the recording was cancelled", async () => {
    const world = setup();
    await recording(world);
    world.dictation.cancel();

    world.recorders[0]?.level(0.9);

    expect(world.dictation.getLevel()).toBe(0);
  });
});

describe("the one-step Undo", () => {
  it("puts the box and the caret back, once", async () => {
    const world = setup({}, "Voir XXX demain");
    await recording(world, { start: 5, end: 8 });
    await finished(world);

    world.dictation.undo();

    expect(world.draft.value).toBe("Voir XXX demain");
    expect(world.dictation.getSnapshot().phase).toBe("idle");
    expect(world.dictation.getSnapshot().caret?.position).toBe(5);
    expect(world.dictation.getSnapshot().caret?.text).toBe("Voir XXX demain");

    world.draft.value = "autre";
    world.dictation.undo();
    expect(world.draft.value).toBe("autre");
  });

  it("does nothing once the box was edited", async () => {
    const world = setup();
    await recording(world);
    await finished(world);
    world.draft.value = "bonjour docteur, merci";

    world.dictation.undo();

    expect(world.draft.value).toBe("bonjour docteur, merci");
    expect(world.dictation.getSnapshot().phase).toBe("done");
  });
});

describe("Escape", () => {
  it.each(["requesting-permission", "recording", "transcribing"] as const)(
    "ends a dictation in the %s phase",
    (phase) => {
      expect(escapeCancelsDictation("Escape", phase, false, false)).toBe(true);
    },
  );

  it("leaves a dialog its own Escape, and the list of names its own", () => {
    expect(escapeCancelsDictation("Escape", "recording", true, false)).toBe(false);
    expect(escapeCancelsDictation("Escape", "recording", false, true)).toBe(false);
  });

  it("does nothing when no dictation is going on, or for another key", () => {
    expect(escapeCancelsDictation("Escape", "idle", false, false)).toBe(false);
    expect(escapeCancelsDictation("Escape", "done", false, false)).toBe(false);
    expect(escapeCancelsDictation("Enter", "recording", false, false)).toBe(false);
  });
});
