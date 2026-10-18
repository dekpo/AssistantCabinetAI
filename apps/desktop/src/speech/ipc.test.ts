import { beforeEach, describe, expect, it, vi } from "vitest";

/* The bridge is replaced by a recorder: these tests read which command the webview asks Rust for and
   how. The names are the contract with `speech_commands.rs`. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { speechCancel, speechTranscribe } from "./ipc";

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("the transcription bridge", () => {
  it("sends the clip as the raw body and the purpose as a header, nothing else", async () => {
    vi.mocked(invoke).mockResolvedValue({
      text: "hello",
      audioMs: 1200,
      processingMs: 300,
      engineId: "fake",
    });
    const wav = new Uint8Array([82, 73, 70, 70]);

    const heard = await speechTranscribe(wav);

    expect(vi.mocked(invoke).mock.calls).toStrictEqual([
      ["speech_transcribe", wav, { headers: { purpose: "dictation" } }],
    ]);
    expect(heard).toStrictEqual({
      text: "hello",
      audioMs: 1200,
      processingMs: 300,
      engineId: "fake",
    });
  });

  it("names a note as the purpose when asked, and never sends a language or a number of seconds", async () => {
    vi.mocked(invoke).mockResolvedValue({});
    const wav = new Uint8Array(0);

    await speechTranscribe(wav, "note");

    const [, body, options] = vi.mocked(invoke).mock.calls[0] as [string, unknown, unknown];
    expect(body).toBe(wav);
    expect(options).toStrictEqual({ headers: { purpose: "note" } });
  });

  it("cancels with the command Rust registers and tells whether a job was running", async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    await expect(speechCancel()).resolves.toBe(true);

    expect(vi.mocked(invoke).mock.calls).toStrictEqual([["speech_cancel"]]);
  });
});
