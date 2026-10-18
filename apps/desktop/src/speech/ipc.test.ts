import { beforeEach, describe, expect, it, vi } from "vitest";

/* The bridge is replaced by a recorder: these tests read which command the webview asks Rust for and
   how. The names are the contract with `speech_commands.rs`. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  speechCancel,
  speechModelCancel,
  speechModelInstall,
  speechModelVerify,
  speechPrepareReading,
  speechTranscribe,
} from "./ipc";

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

describe("the model bridge", () => {
  it("hands the progress channel to Rust and forwards what arrives on it", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    const seen: unknown[] = [];

    await speechModelInstall((progress) => seen.push(progress));

    const [command, arguments_] = vi.mocked(invoke).mock.calls[0] as [
      string,
      { onProgress: { onmessage: (progress: unknown) => void } },
    ];
    expect(command).toBe("speech_model_install");
    expect(Object.keys(arguments_)).toStrictEqual(["onProgress"]);
    arguments_.onProgress.onmessage({ phase: "download", doneBytes: 10, totalBytes: 20 });
    expect(seen).toStrictEqual([{ phase: "download", doneBytes: 10, totalBytes: 20 }]);
  });

  it("asks for the check and the stop without sending a path, an address or a digest", async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    await speechModelVerify();
    await speechModelCancel();

    expect(vi.mocked(invoke).mock.calls).toStrictEqual([
      ["speech_model_verify"],
      ["speech_model_cancel"],
    ]);
  });
});

describe("the reading bridge", () => {
  it("sends the parts and the locale under their own names, and nothing else", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    const parts = [{ kind: "markdown" as const, text: "**Bonjour**" }];

    await speechPrepareReading(parts, "fr-FR");

    expect(vi.mocked(invoke).mock.calls).toStrictEqual([
      ["speech_prepare_reading", { parts, locale: "fr-FR" }],
    ]);
    expect(Object.keys(vi.mocked(invoke).mock.calls[0]?.[1] as object)).toStrictEqual([
      "parts",
      "locale",
    ]);
  });

  it("hands back the items as Rust answered them", async () => {
    const items = [
      { kind: "speech", text: "Bonjour.", pauseAfter: "sentence" },
      { kind: "note", code: "table_omitted", count: 12 },
    ];
    vi.mocked(invoke).mockResolvedValue(items);

    await expect(speechPrepareReading([], "fr-FR")).resolves.toStrictEqual(items);
  });
});
