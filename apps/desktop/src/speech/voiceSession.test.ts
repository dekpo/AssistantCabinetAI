import { describe, expect, it, vi } from "vitest";
import { createVoiceSession } from "./voiceSession";

describe("one microphone at a time", () => {
  it("grants the first recorder and refuses a second one with its sentence", () => {
    const session = createVoiceSession();

    expect(session.claim("dictation", () => {})).toStrictEqual({ granted: true });
    expect(session.claim("notes", () => {})).toStrictEqual({
      granted: false,
      reasonKey: "speech.session.microphoneBusy",
    });
    expect(session.holder()).toBe("dictation");
  });

  it("lets the second recorder in once the first has released", () => {
    const session = createVoiceSession();
    session.claim("dictation", () => {});
    session.release("dictation");

    expect(session.claim("notes", () => {})).toStrictEqual({ granted: true });
    expect(session.holder()).toBe("notes");
  });

  it("stops a playback in progress when a recording starts", () => {
    const session = createVoiceSession();
    const stopPlayback = vi.fn(() => session.release("readAloud"));
    session.claim("readAloud", stopPlayback);

    expect(session.claim("dictation", () => {})).toStrictEqual({ granted: true });
    expect(stopPlayback).toHaveBeenCalledTimes(1);
    expect(session.holder()).toBe("dictation");
  });

  it("does not start a playback while something records", () => {
    const session = createVoiceSession();
    session.claim("notes", () => {});

    expect(session.claim("readAloud", () => {})).toStrictEqual({
      granted: false,
      reasonKey: "speech.session.recording",
    });
    expect(session.holder()).toBe("notes");
  });

  it("does not let a second playback replace the first", () => {
    const session = createVoiceSession();
    const stop = vi.fn();
    session.claim("readAloud", stop);

    expect(session.claim("readAloud", () => {})).toStrictEqual({ granted: true });
    expect(stop).not.toHaveBeenCalled();
    expect(session.holder()).toBe("readAloud");
  });

  it("ignores the late release of an interrupted playback", () => {
    const session = createVoiceSession();
    session.claim("readAloud", () => {});
    session.claim("dictation", () => {});

    session.release("readAloud");

    expect(session.holder()).toBe("dictation");
  });

  it("tells its listeners when the holder changes, and stops when they leave", () => {
    const session = createVoiceSession();
    const listener = vi.fn();
    const leave = session.subscribe(listener);

    session.claim("dictation", () => {});
    session.release("dictation");
    expect(listener).toHaveBeenCalledTimes(2);

    leave();
    session.claim("notes", () => {});
    expect(listener).toHaveBeenCalledTimes(2);
  });
});
