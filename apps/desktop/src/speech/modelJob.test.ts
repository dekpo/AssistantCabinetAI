import { describe, expect, it } from "vitest";
import {
  afterFailure,
  IDLE_JOB,
  installing,
  jobLine,
  modelButtons,
  percentOf,
  type ModelJob,
} from "./modelJob";

describe("the progress of an install", () => {
  it("is a whole percent, bounded, and zero while the total is unknown", () => {
    expect(percentOf({ phase: "download", doneBytes: 0, totalBytes: 0 })).toBe(0);
    expect(percentOf({ phase: "download", doneBytes: 1, totalBytes: 3 })).toBe(33);
    expect(percentOf({ phase: "download", doneBytes: 999, totalBytes: 1_000 })).toBe(99);
    expect(percentOf({ phase: "unpack", doneBytes: 5, totalBytes: 5 })).toBe(100);
    expect(percentOf({ phase: "unpack", doneBytes: 9, totalBytes: 5 })).toBe(100);
    expect(percentOf({ phase: "unpack", doneBytes: -4, totalBytes: 5 })).toBe(0);
  });

  it("names the phase it is in", () => {
    const half = installing({ phase: "download", doneBytes: 50, totalBytes: 100 });
    expect(half).toStrictEqual({ kind: "installing", phase: "download", percent: 50 });
    expect(jobLine(half)).toStrictEqual({
      key: "speech.install.downloading",
      values: { percent: 50 },
    });
    expect(jobLine(installing({ phase: "unpack", doneBytes: 10, totalBytes: 100 }))?.key).toBe(
      "speech.install.unpacking",
    );
    expect(jobLine({ kind: "verifying" })?.key).toBe("speech.install.verifying");
    expect(jobLine(IDLE_JOB)).toBeNull();
    expect(jobLine({ kind: "failed", error: { code: "x", data: {} } })).toBeNull();
  });
});

describe("what a failed job leaves on screen", () => {
  it("leaves nothing after a cancel, and the error after any other failure", () => {
    expect(afterFailure({ code: "speech_cancelled", data: {} })).toStrictEqual(IDLE_JOB);
    const error = { code: "speech_model_download_failed", data: { folder: "f" } };
    expect(afterFailure(error)).toStrictEqual({ kind: "failed", error });
  });
});

describe("the buttons of the model line", () => {
  const idle: ModelJob = IDLE_JOB;
  const running: ModelJob = { kind: "installing", phase: "download", percent: 1 };

  it("installs a model that is missing or damaged, and verifies one that is not missing", () => {
    expect(modelButtons(idle, "missing", true)).toStrictEqual({
      installDisabled: false,
      verifyDisabled: true,
      cancelShown: false,
    });
    expect(modelButtons(idle, "corrupt", true)).toStrictEqual({
      installDisabled: false,
      verifyDisabled: false,
      cancelShown: false,
    });
    expect(modelButtons(idle, "unverified", true)).toStrictEqual({
      installDisabled: true,
      verifyDisabled: false,
      cancelShown: false,
    });
    expect(modelButtons(idle, "present", true).installDisabled).toBe(true);
  });

  it("allows nothing but Cancel while a job runs", () => {
    for (const model of ["missing", "unverified", "present", "corrupt"] as const) {
      expect(modelButtons(running, model, true)).toStrictEqual({
        installDisabled: true,
        verifyDisabled: true,
        cancelShown: true,
      });
    }
    expect(modelButtons({ kind: "verifying" }, "unverified", true).cancelShown).toBe(true);
  });

  it("allows nothing while the pieces behind it do not exist", () => {
    expect(modelButtons(idle, "missing", false)).toStrictEqual({
      installDisabled: true,
      verifyDisabled: true,
      cancelShown: false,
    });
  });
});
