import { describe, expect, it } from "vitest";
import type { FeatureStatus, SpeechStatus } from "../ipc";
import { chooseEngine, readAloudVisibility } from "./engineChoice";

const READY: FeatureStatus = { available: true, reason: null };
const blocked = (reason: string): FeatureStatus => ({ available: false, reason });

function status(readAloud: FeatureStatus, enabled = true): SpeechStatus {
  return {
    enabled,
    locale: "fr-FR",
    features: { dictation: READY, readAloud, notes: READY },
    sttModel: "present",
    voices: [],
    languages: [],
  };
}

describe("visibility matrix: when the read-aloud button is drawn", () => {
  it("is hidden until Rust has answered", () => {
    expect(readAloudVisibility(null, "system")).toStrictEqual({ visible: false, engine: "none" });
  });

  it("is hidden with the voice master switch off, whatever the engine setting", () => {
    expect(readAloudVisibility(status(READY, false), "system").visible).toBe(false);
    expect(readAloudVisibility(status(blocked("speech_unavailable"), false), null).visible).toBe(false);
  });

  it("is hidden when the read-aloud switch is off or the language has no read-aloud", () => {
    expect(readAloudVisibility(status(blocked("speech_unavailable")), "system").visible).toBe(false);
    expect(readAloudVisibility(status(blocked("speech_locale_unsupported")), "system").visible).toBe(false);
  });

  it("is drawn with no voice installed, so pressing it can say why", () => {
    expect(readAloudVisibility(status(blocked("tts_no_local_voice")), null)).toStrictEqual({
      visible: true,
      engine: "none",
    });
  });

  it("is drawn with the system voices only when the user chose them", () => {
    expect(readAloudVisibility(status(blocked("tts_no_local_voice")), "system")).toStrictEqual({
      visible: true,
      engine: "system",
    });
    expect(readAloudVisibility(status(blocked("tts_no_local_voice")), "piper").engine).toBe("none");
  });

  it("uses the Piper worker when Rust found a voice for the language", () => {
    expect(readAloudVisibility(status(READY), null)).toStrictEqual({ visible: true, engine: "piper" });
    expect(readAloudVisibility(status(READY), "piper").engine).toBe("piper");
  });
});

describe("which engine reads", () => {
  it("never falls back to the system voices on its own", () => {
    expect(chooseEngine(status(blocked("tts_no_local_voice")), null)).toBe("none");
    expect(chooseEngine(null, null)).toBe("none");
    expect(chooseEngine(status(blocked("tts_no_local_voice")), "piper")).toBe("none");
  });

  it("honours an explicit choice of the system voices even beside a Piper voice", () => {
    expect(chooseEngine(status(READY), "system")).toBe("system");
  });
});
