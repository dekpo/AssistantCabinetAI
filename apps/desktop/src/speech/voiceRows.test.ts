import { describe, expect, it } from "vitest";
import type { SpeechStatus } from "./ipc";
import { voiceRows } from "./voiceRows";

const status = (overrides: Partial<SpeechStatus>): SpeechStatus => ({
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
    {
      id: "en-US",
      support: "enabled",
      dictation: true,
      readAloud: true,
      voices: [{ gender: "female", id: "ljspeech" }],
    },
  ],
  voices: [
    { id: "siwis", state: "present", rating: "review", bytes: 67_428_923 },
    { id: "css10", state: "missing", rating: "attribution", bytes: 65_346_319 },
    { id: "ljspeech", state: "corrupt", rating: "clear", bytes: 67_436_754 },
  ],
  ...overrides,
});

describe("the voice rows of the settings", () => {
  it("lists the voices of the language in the table's order, with state, rating and size", () => {
    expect(voiceRows(status({}))).toStrictEqual([
      { id: "siwis", gender: "female", state: "present", rating: "review", megabytes: 67 },
      { id: "css10", gender: "male", state: "missing", rating: "attribution", megabytes: 65 },
    ]);
  });

  it("follows the language speech works in", () => {
    expect(voiceRows(status({ locale: "en-US" })).map((row) => row.id)).toStrictEqual(["ljspeech"]);
  });

  it("never leaves the rating out when Rust reported it, even for a voice not installed", () => {
    for (const row of voiceRows(status({}))) {
      expect(row.rating).not.toBeNull();
    }
  });

  it("calls a voice Rust did not report missing, with no rating and no size", () => {
    expect(voiceRows(status({ voices: [] }))).toStrictEqual([
      { id: "siwis", gender: "female", state: "missing", rating: null, megabytes: 0 },
      { id: "css10", gender: "male", state: "missing", rating: null, megabytes: 0 },
    ]);
  });

  it("is empty before Rust answered and for a language the table does not have", () => {
    expect(voiceRows(null)).toStrictEqual([]);
    expect(voiceRows(status({ locale: "it-IT" }))).toStrictEqual([]);
  });
});
