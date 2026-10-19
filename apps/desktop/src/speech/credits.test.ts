import { readFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { SpeechCredits } from "../components/SpeechCredits";
import { CATALOGUES, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { I18nProvider } from "../i18n/I18nProvider";
import { lookupKey, type Catalogue } from "../i18n/translate";
import { COMPONENT_CREDITS, voiceCreditKey, voiceCredits } from "./credits";
import type { SpeechStatus } from "./ipc";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

const manifest = JSON.parse(
  readFileSync(
    new URL("../../src-tauri/resources/speech/models-manifest.json", import.meta.url),
    "utf8",
  ),
) as { models: { id: string; kind: string; voice?: { rating: string } }[] };
const MANIFEST_VOICES = manifest.models.filter((model) => model.kind === "tts");

const STATUS = {
  enabled: true,
  locale: "fr-FR",
  features: {},
  sttModel: "present",
  languages: [],
  voices: MANIFEST_VOICES.map((voice) => ({
    id: voice.id,
    state: "missing",
    rating: voice.voice?.rating,
    bytes: 1,
  })),
} as unknown as SpeechStatus;

function draw(status: SpeechStatus | null, locale = "fr-FR"): string {
  return renderToStaticMarkup(
    createElement(I18nProvider, {
      locale,
      children: createElement(SpeechCredits, { status }),
    }),
  );
}

describe("the credits of the voice", () => {
  it("has a sentence, in both languages, for every component and every voice of the manifest", () => {
    expect(MANIFEST_VOICES.length).toBeGreaterThanOrEqual(4);
    const keys = [
      ...COMPONENT_CREDITS.map((credit) => credit.textKey),
      ...MANIFEST_VOICES.map((voice) => voiceCreditKey(voice.id)),
      "speech.credits.title",
      "speech.credits.intro",
      "speech.credits.componentsLabel",
      "speech.credits.voicesLabel",
    ];
    for (const locale of SUPPORTED_LOCALES) {
      const catalogue = CATALOGUES[locale] as Catalogue;
      for (const key of keys) {
        expect(lookupKey(catalogue, key), `${locale} ${key}`).toBeTruthy();
      }
    }
  });

  it("states the Parakeet attribution the licence asks for, in both languages", () => {
    for (const locale of SUPPORTED_LOCALES) {
      const markup = draw(STATUS, locale);

      expect(markup).toContain("NVIDIA Parakeet TDT 0.6B v3 (nvidia/parakeet-tdt-0.6b-v3)");
      expect(markup).toContain("© NVIDIA");
      expect(markup).toContain("https://creativecommons.org/licenses/by/4.0/");
      expect(markup).toContain("int8");
    }
  });

  it("credits sherpa-onnx, ONNX Runtime and the espeak-ng notice, and says where the phonemizer data is", () => {
    const markup = draw(STATUS, "en-US");

    expect(markup).toContain("sherpa-onnx");
    expect(markup).toContain("Apache-2.0");
    expect(markup).toContain("ONNX Runtime");
    expect(markup).toContain("espeak-ng");
    expect(markup).toContain("GPL-3.0");
    expect(markup).toContain("speech-tts-worker-NOTICE.md");
    expect(markup).toContain("each voice brings its own copy");
  });

  it("lists every voice of the manifest with its licence rating, none left out", () => {
    const markup = draw(STATUS, "en-US");

    for (const voice of MANIFEST_VOICES) {
      expect(markup, voice.id).toContain(voice.id);
    }
    // The voice whose licence still has to be read says so in its row.
    const siwis = markup.slice(markup.indexOf("SIWIS"));
    expect(siwis).toContain("its licence must be checked before any wider distribution");
    expect(markup).toContain("Free to use, with a credit to its authors.");
    expect(markup).toContain("Free to use.");
  });

  it("follows the rating Rust reports, and shows no voice list before Rust answered", () => {
    expect(voiceCredits(null)).toStrictEqual([]);
    const rows = voiceCredits(STATUS);
    expect(rows.map((row) => row.id)).toStrictEqual(MANIFEST_VOICES.map((voice) => voice.id));
    expect(rows.find((row) => row.id === "siwis")?.rating).toBe("review");

    const markup = draw(null, "en-US");
    expect(markup).toContain("Parakeet");
    expect(markup).not.toContain("Coqui VITS css10");
    expect(markup).not.toContain("Voices</span>");
  });

  it("draws a closed block", () => {
    expect(draw(null)).toContain('<details class="field settings-credits">');
    expect(draw(null)).not.toContain("<details open");
  });
});
