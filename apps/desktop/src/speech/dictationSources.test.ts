import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { lookupKey, type Catalogue } from "../i18n/translate";
import { CLIPPED_SHARE_WARNING, MIN_DICTATION_SECONDS, QUIET_PEAK_FLOOR } from "./dictation";

const read = (relative: string) => readFileSync(new URL(relative, import.meta.url), "utf8");

/** Source without its comments, so a sentence about "sending" is not taken for a call to send. */
function code(relative: string): string {
  return read(relative)
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .split("\n")
    .filter((line) => !line.trim().startsWith("//"))
    .join("\n");
}

const FILES = [
  "./dictation.ts",
  "./insertion.ts",
  "./useDictation.ts",
  "../components/MicButton.tsx",
  "../components/DictationStatus.tsx",
];

describe("a dictation only fills the box", () => {
  it.each(FILES)("%s never sends, submits or touches the conversation", (file) => {
    const source = code(file);

    expect(source).not.toMatch(/\b(send|onSend|submit|useChat|regenerate|resend)\b/);
  });

  it("is handed to the composer as a property that holds no send of its own", () => {
    const composer = read("../components/Composer.tsx");
    const mic = composer.slice(composer.indexOf("<MicButton"), composer.indexOf("/>", composer.indexOf("<MicButton")));

    expect(mic).not.toMatch(/onSend|submit|onStop/);
    expect(mic).toContain("dictation.start");
  });

  it("is ended by the chat panel before a question is sent, stopped, or the conversation cleared", () => {
    const panel = read("../components/ChatPanel.tsx");

    expect(panel.match(/dictation\?\.cancel\(\)/g)?.length).toBe(3);
    expect(panel).toMatch(/onSend=\{\(question\) => \{\s*dictation\?\.cancel\(\);/);
    expect(panel).toMatch(/const onStop = \(\) => \{\s*dictation\?\.cancel\(\);/);
    expect(panel).toMatch(/onConfirm=\{\(\) => \{\s*dictation\?\.cancel\(\);\s*clear\(\);/);
  });
});

describe("the dictation reaches nothing but the speech commands", () => {
  it.each(FILES)("%s opens no connection, keeps no audio and writes nothing", (file) => {
    const source = code(file);

    expect(source).not.toMatch(
      /https?:|fetch\(|XMLHttpRequest|WebSocket|sendBeacon|localStorage|sessionStorage|indexedDB|createObjectURL|new Blob|writeFile|@tauri-apps/,
    );
  });

  it("calls Rust only through the speech bridge", () => {
    const source = code("./useDictation.ts");
    const calls = [...source.matchAll(/\b(speech[A-Z][A-Za-z]+)\(/g)].map(([, name]) => name);

    expect([...new Set(calls)].sort()).toEqual(["speechCancel", "speechTranscribe"]);
  });

  it("sends neither a language nor a length: Rust reads them from the settings", () => {
    const source = code("./useDictation.ts");

    expect(source).toContain('speechTranscribe(wav, "dictation")');
    expect(source).not.toMatch(/language|locale:/);
  });
});

describe("the numbers the webview checks are the ones Rust enforces", () => {
  const wav = readFileSync(new URL("../../src-tauri/src/speech/wav.rs", import.meta.url), "utf8");

  it("the quiet floor is the peak floor of the Rust parser", () => {
    const rust = Number(/QUIET_PEAK_FLOOR: f32 = ([0-9.]+);/.exec(wav)?.[1]);

    expect(QUIET_PEAK_FLOOR).toBe(rust);
  });

  it("a clip refused here is never one Rust would have accepted", () => {
    const rustMilliseconds = Number(/MIN_CLIP_MS: u64 = ([0-9_]+);/.exec(wav)?.[1]?.replace("_", ""));

    expect(MIN_DICTATION_SECONDS * 1000).toBeGreaterThanOrEqual(rustMilliseconds);
    expect(CLIPPED_SHARE_WARNING).toBeGreaterThan(0);
  });
});

describe("the sentences of the dictation", () => {
  const keys = [
    ...new Set(
      FILES.flatMap((file) =>
        [...read(file).matchAll(/"((?:speech|errors)\.[A-Za-z_.]+)"/g)].map(([, key]) => key as string),
      ),
    ),
  ];
  /* Keys built from a code or a failure name, which a scan of the sources cannot see. */
  const BUILT = [
    "speech.microphone.denied",
    "speech.microphone.missing",
    "speech.microphone.unsupported",
    "speech.microphone.failed",
    "speech.dictation.numbersOne",
    "speech.dictation.numbersMany",
    "errors.speech_unavailable",
    "errors.speech_engine_missing",
    "errors.speech_model_missing",
    "errors.speech_model_corrupt",
    "errors.speech_locale_unsupported",
    "errors.speech_busy",
    "errors.speech_audio_invalid",
    "errors.speech_audio_too_short",
    "errors.speech_audio_too_quiet",
    "errors.speech_audio_too_long",
    "errors.speech_timeout",
    "errors.speech_engine_failed",
    "errors.speech_output_rejected",
    "errors.unknown_error",
  ];

  it("finds the keys the sources name", () => {
    expect(keys).toContain("speech.dictation.nothingHeard");
    expect(keys).toContain("errors.speech_audio_too_short");
  });

  it.each(SUPPORTED_LOCALES)("%s has a sentence for each of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of [...new Set([...keys, ...BUILT])]) {
      expect(lookupKey(catalogue, key), key).toBeTruthy();
    }
  });
});
