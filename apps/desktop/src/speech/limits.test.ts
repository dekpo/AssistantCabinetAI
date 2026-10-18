import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { SPEECH_LIMITS } from "./limits";

/** A numeric constant of `speech/mod.rs`, underscores and all (`3_600`). */
function rustConstant(name: string): number {
  const source = readFileSync(new URL("../../src-tauri/src/speech/mod.rs", import.meta.url), "utf8");
  const match = new RegExp(`pub const ${name}: [a-z0-9]+ = ([0-9_.]+);`).exec(source);
  if (match === null) {
    throw new Error(`constant not found: ${name}`);
  }
  return Number((match[1] as string).replaceAll("_", ""));
}

describe("the limits of the speech controls", () => {
  it("are the ones Rust clamps to", () => {
    expect(SPEECH_LIMITS.dictationSeconds).toStrictEqual({
      min: rustConstant("MIN_DICTATION_SECONDS"),
      max: rustConstant("MAX_DICTATION_SECONDS"),
    });
    expect(SPEECH_LIMITS.noteSeconds).toStrictEqual({
      min: rustConstant("MIN_NOTE_SECONDS"),
      max: rustConstant("MAX_NOTE_SECONDS"),
    });
    expect(SPEECH_LIMITS.ttsRate).toStrictEqual({
      min: rustConstant("MIN_TTS_RATE"),
      max: rustConstant("MAX_TTS_RATE"),
    });
  });
});
