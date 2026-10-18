import { describe, expect, it } from "vitest";
import { CATALOGUES } from "../i18n/catalogues";
import { lookupKey, type Catalogue } from "../i18n/translate";
import { DEFAULT_SPEECH_SETTINGS } from "./ipc";

/** The machine codes of the speech module (`docs/SPEECH.md`, section 6), in `error.rs`. */
const SPEECH_CODES = [
  "speech_unavailable",
  "speech_engine_missing",
  "speech_model_missing",
  "speech_model_corrupt",
  "speech_audio_too_short",
  "speech_audio_too_quiet",
  "speech_audio_too_long",
  "speech_audio_invalid",
  "speech_busy",
  "speech_cancelled",
  "speech_timeout",
  "speech_engine_failed",
  "speech_locale_unsupported",
  "speech_output_rejected",
  "tts_no_local_voice",
  "tts_failed",
  "notes_folder_unavailable",
  "notes_name_invalid",
  "notes_too_large",
  "notes_too_many",
  "notes_write_failed",
  "notes_analysis_failed",
];

function placeholders(sentence: string): string[] {
  return [...sentence.matchAll(/\{(\w+)\}/g)].map(([, name]) => name as string).sort();
}

describe("the sentences of the speech codes", () => {
  const french = CATALOGUES["fr-FR"] as Catalogue;
  const english = CATALOGUES["en-US"] as Catalogue;

  it.each(SPEECH_CODES)("%s has a sentence in both languages with the same values", (code) => {
    const fr = lookupKey(french, `errors.${code}`);
    const en = lookupKey(english, `errors.${code}`);

    expect(fr, `fr-FR ${code}`).toBeTruthy();
    expect(en, `en-US ${code}`).toBeTruthy();
    expect(fr).not.toBe(en);
    expect(placeholders(fr ?? "")).toStrictEqual(placeholders(en ?? ""));
  });

  it("is the whole list: the rust file names exactly these speech codes", async () => {
    const { readFileSync } = await import("node:fs");
    const source = readFileSync(
      new URL("../../src-tauri/src/error.rs", import.meta.url),
      "utf8",
    );
    const named = [...source.matchAll(/=>\s*"((?:speech|tts|notes)_[a-z_]+)"/g)].map(
      ([, code]) => code as string,
    );

    expect(named.sort()).toStrictEqual([...SPEECH_CODES].sort());
  });

  it("gives the unsupported language its value", () => {
    expect(placeholders(lookupKey(english, "errors.speech_locale_unsupported") ?? "")).toStrictEqual([
      "locale",
    ]);
  });
});

describe("the speech settings the interface starts from", () => {
  it("keeps voice off and notes on, like Rust", () => {
    expect(DEFAULT_SPEECH_SETTINGS.enabled).toBe(false);
    expect(DEFAULT_SPEECH_SETTINGS.notesEnabled).toBe(true);
    expect(DEFAULT_SPEECH_SETTINGS.languageOverride).toBeNull();
    expect(DEFAULT_SPEECH_SETTINGS.maxDictationSeconds).toBe(120);
    expect(DEFAULT_SPEECH_SETTINGS.maxNoteSeconds).toBe(1800);
    expect(DEFAULT_SPEECH_SETTINGS.notesFolderName).toBe("Notes");
    // The address is Rust's: the webview holds none.
    expect(DEFAULT_SPEECH_SETTINGS.speechModelsUrl).toBe("");
  });
});
