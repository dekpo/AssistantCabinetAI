import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  boundedNumber,
  fractionFromPercentDraft,
  MAX_CANDIDATES,
  MAX_SELECTIVITY,
  MIN_REDUCTION,
  MIN_SCOPE_FILES,
  percentDraft,
  WIDEN_BELOW_SCORE,
} from "./knowledgeSettings";

const SETTINGS_RS = resolve(
  fileURLToPath(new URL(".", import.meta.url)),
  "../../src-tauri/src/settings.rs",
);

describe("the limits of the targeting settings", () => {
  it("are the ones Rust clamps to, read from its source", () => {
    const source = readFileSync(SETTINGS_RS, "utf8");

    // `targeting_policy`: the single place the five numbers are bounded on the way out.
    expect(source).toMatch(/knowledge_min_scope_files\s*\.clamp\(1, 10_000\)/);
    expect(source).toMatch(/\.clamp\(0\.05, 1\.0\)/);
    expect(source).toMatch(/\.clamp\(0\.0, 0\.95\)/);
    expect(source).toMatch(/knowledge_max_candidates\s*\.clamp\(1, 500\)/);
    expect(source).toMatch(/\.clamp\(0\.0, 1\.5\)/);

    expect(MIN_SCOPE_FILES).toStrictEqual({ min: 1, max: 10_000 });
    expect(MAX_SELECTIVITY).toStrictEqual({ min: 0.05, max: 1 });
    expect(MIN_REDUCTION).toStrictEqual({ min: 0, max: 0.95 });
    expect(MAX_CANDIDATES).toStrictEqual({ min: 1, max: 500 });
    expect(WIDEN_BELOW_SCORE).toStrictEqual({ min: 0, max: 1.5 });
  });
});

describe("a number typed in the settings dialog", () => {
  it("is kept inside its limit, as a whole number when it is one", () => {
    expect(boundedNumber("12", MIN_SCOPE_FILES, 8)).toBe(12);
    expect(boundedNumber("0", MIN_SCOPE_FILES, 8)).toBe(1);
    expect(boundedNumber("99999", MIN_SCOPE_FILES, 8)).toBe(10_000);
    expect(boundedNumber("12.6", MIN_SCOPE_FILES, 8)).toBe(13);
    expect(boundedNumber("-4", MAX_CANDIDATES, 15)).toBe(1);
  });

  it("keeps the stored value when the field is empty or is not a number", () => {
    expect(boundedNumber("", MIN_SCOPE_FILES, 8)).toBe(8);
    expect(boundedNumber("   ", MIN_SCOPE_FILES, 8)).toBe(8);
    expect(boundedNumber("abc", MIN_SCOPE_FILES, 8)).toBe(8);
    expect(boundedNumber("Infinity", MIN_SCOPE_FILES, 8)).toBe(8);
  });

  it("reads a comma as a decimal point and keeps two decimals of a score", () => {
    expect(boundedNumber("0,65", WIDEN_BELOW_SCORE, 0.65, 2)).toBe(0.65);
    expect(boundedNumber("0.6789", WIDEN_BELOW_SCORE, 0.65, 2)).toBe(0.68);
    expect(boundedNumber("3", WIDEN_BELOW_SCORE, 0.65, 2)).toBe(1.5);
  });
});

describe("a share shown as a percentage", () => {
  it("is shown whole and stored as a fraction", () => {
    expect(percentDraft(0.6)).toBe("60");
    expect(percentDraft(0.3)).toBe("30");
    expect(fractionFromPercentDraft("45", MAX_SELECTIVITY, 0.6)).toBe(0.45);
  });

  it("is pulled back inside its limit", () => {
    expect(fractionFromPercentDraft("0", MAX_SELECTIVITY, 0.6)).toBe(0.05);
    expect(fractionFromPercentDraft("250", MAX_SELECTIVITY, 0.6)).toBe(1);
    expect(fractionFromPercentDraft("99", MIN_REDUCTION, 0.3)).toBe(0.95);
    expect(fractionFromPercentDraft("-5", MIN_REDUCTION, 0.3)).toBe(0);
  });

  it("keeps the stored share when the field is empty", () => {
    expect(fractionFromPercentDraft("", MAX_SELECTIVITY, 0.6)).toBe(0.6);
    expect(fractionFromPercentDraft("abc", MIN_REDUCTION, 0.3)).toBe(0.3);
  });
});
