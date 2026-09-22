import { describe, expect, it } from "vitest";
import {
  canDiscard,
  canInterrupt,
  phaseAfterFirstText,
  stopOutcome,
  STOPPED_CODE,
  wasStopped,
  type GenerationPhase,
} from "./generation";

const PHASES: GenerationPhase[] = ["idle", "thinking", "writing"];

describe("what a stop leaves behind", () => {
  it("abandons the turn while nothing has been written", () => {
    expect(stopOutcome("thinking")).toBe("discardTurn");
  });

  // The decision that makes the second stop safe: a half-written answer is never taken away from
  // her, it is only stopped growing.
  it("keeps the words already on screen once the answer is being written", () => {
    expect(stopOutcome("writing")).toBe("keepPartialAnswer");
  });

  it("has nothing to stop when nothing is running", () => {
    expect(stopOutcome("idle")).toBeNull();
  });
});

describe("the two stops never appear at the same time", () => {
  it("offers exactly one of them per phase, and none when idle", () => {
    expect(PHASES.filter(canDiscard)).toStrictEqual(["thinking"]);
    expect(PHASES.filter(canInterrupt)).toStrictEqual(["writing"]);
  });

  it.each(PHASES)("%s never offers both", (phase) => {
    expect(canDiscard(phase) && canInterrupt(phase)).toBe(false);
  });
});

describe("phaseAfterFirstText", () => {
  it("moves from thinking to writing", () => {
    expect(phaseAfterFirstText("thinking")).toBe("writing");
  });

  it("leaves a run that is already writing alone", () => {
    expect(phaseAfterFirstText("writing")).toBe("writing");
  });

  // A delta arriving after the run ended must not make the composer believe it is busy again.
  it("cannot revive a finished run", () => {
    expect(phaseAfterFirstText("idle")).toBe("idle");
  });
});

describe("wasStopped", () => {
  it("recognises her own stop", () => {
    expect(wasStopped(STOPPED_CODE)).toBe(true);
  });

  it("treats a real failure as a failure", () => {
    expect(wasStopped("server_unreachable")).toBe(false);
    expect(wasStopped("insufficient_evidence")).toBe(false);
  });
});
