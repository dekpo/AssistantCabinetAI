import { describe, expect, it } from "vitest";
import {
  featureAvailability,
  installButton,
  modelStatusKey,
  overrideLanguages,
  voicesOfResolvedLanguage,
} from "./availability";
import type { FeatureStatus, SpeechFeature, SpeechStatus } from "./ipc";

const READY: FeatureStatus = { available: true, reason: null };

function blocked(reason: string): FeatureStatus {
  return { available: false, reason };
}

function statusWith(features: Partial<Record<SpeechFeature, FeatureStatus>>): SpeechStatus {
  return {
    enabled: true,
    locale: "fr-FR",
    features: {
      dictation: READY,
      readAloud: READY,
      notes: READY,
      ...features,
    },
    sttModel: "present",
    voices: [],
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
      { id: "en-US", support: "enabled", dictation: true, readAloud: true, voices: [] },
      { id: "de-DE", support: "planned", dictation: false, readAloud: false, voices: [] },
    ],
  };
}

describe("what a feature says about being available", () => {
  it("says nothing before Rust has answered", () => {
    expect(featureAvailability(null, "dictation", "supported")).toStrictEqual({
      state: "unknown",
      reasonKey: null,
    });
  });

  it("is ready when Rust says so and the microphone is there", () => {
    expect(featureAvailability(statusWith({}), "dictation", "supported")).toStrictEqual({
      state: "ready",
      reasonKey: null,
    });
  });

  it("takes a switched-off feature for a choice, not a problem", () => {
    const status = statusWith({ dictation: blocked("speech_unavailable") });

    expect(featureAvailability(status, "dictation", "supported")).toStrictEqual({
      state: "off",
      reasonKey: null,
    });
  });

  it.each([
    "speech_locale_unsupported",
    "speech_engine_missing",
    "speech_model_missing",
    "speech_model_corrupt",
    "tts_no_local_voice",
    "notes_folder_unavailable",
  ])("maps the reason %s to the sentence that exists for it", (code) => {
    const status = statusWith({ dictation: blocked(code) });

    expect(featureAvailability(status, "dictation", "supported")).toStrictEqual({
      state: "blocked",
      reasonKey: `errors.${code}`,
    });
  });

  it("blocks dictation, and only dictation, when the window has no microphone", () => {
    const status = statusWith({});

    expect(featureAvailability(status, "dictation", "unsupported").reasonKey).toBe(
      "speech.microphone.unsupported",
    );
    expect(featureAvailability(status, "readAloud", "unsupported").state).toBe("ready");
    expect(featureAvailability(status, "notes", "unsupported").state).toBe("ready");
  });

  it("puts the engine's reason before the microphone's", () => {
    const status = statusWith({ dictation: blocked("speech_model_missing") });

    expect(featureAvailability(status, "dictation", "unsupported").reasonKey).toBe(
      "errors.speech_model_missing",
    );
  });

  it("keeps notes available while the master switch is off, as Rust reports them", () => {
    const status = { ...statusWith({ dictation: blocked("speech_unavailable") }), enabled: false };

    expect(featureAvailability(status, "notes", "supported").state).toBe("ready");
  });
});

describe("the model line and the Install button", () => {
  it.each(["missing", "unverified", "present", "corrupt"] as const)("has a sentence key for %s", (state) => {
    expect(modelStatusKey(state)).toBe(`speech.model.${state}`);
  });

  it("disables Install with its reason while the download does not exist", () => {
    expect(installButton(false)).toStrictEqual({
      disabled: true,
      reasonKey: "speech.install.notYet",
    });
    expect(installButton(true)).toStrictEqual({ disabled: false, reasonKey: null });
  });
});

describe("the choices that come from the language table", () => {
  it("offers only the enabled languages for the override", () => {
    expect(overrideLanguages(statusWith({})).map((language) => language.id)).toStrictEqual([
      "fr-FR",
      "en-US",
    ]);
    expect(overrideLanguages(null)).toStrictEqual([]);
  });

  it("offers the voices of the language speech resolved to", () => {
    expect(voicesOfResolvedLanguage(statusWith({})).map((voice) => voice.id)).toStrictEqual([
      "siwis",
      "css10",
    ]);
    expect(voicesOfResolvedLanguage(null)).toStrictEqual([]);
    expect(voicesOfResolvedLanguage({ ...statusWith({}), locale: "it-IT" })).toStrictEqual([]);
  });
});
