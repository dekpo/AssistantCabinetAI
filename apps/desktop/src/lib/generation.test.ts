import { describe, expect, it } from "vitest";
import {
  canStop,
  phaseAfterFirstText,
  STOPPED_CODE,
  wasStopped,
  type GenerationPhase,
} from "./generation";

describe("canStop", () => {
  it("offers the stop while the question is still being thought about", () => {
    expect(canStop("thinking")).toBe(true);
  });

  // The decision this whole module exists for: a half-read answer is never taken away from her.
  it("does not offer it once the answer is being written", () => {
    expect(canStop("writing")).toBe(false);
  });

  it("does not offer it when nothing is running", () => {
    expect(canStop("idle")).toBe(false);
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

describe("the phases are exhaustive", () => {
  it("has a stop rule for every phase", () => {
    const phases: GenerationPhase[] = ["idle", "thinking", "writing"];

    expect(phases.filter(canStop)).toStrictEqual(["thinking"]);
  });
});
