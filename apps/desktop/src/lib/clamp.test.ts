import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CLAMP_LINES, overflowsClamp } from "./clamp";

describe("overflowsClamp", () => {
  it("shows no toggle on a question that fits", () => {
    expect(overflowsClamp(80, 80)).toBe(false);
  });

  it("shows a toggle once the text is taller than the clamp", () => {
    expect(overflowsClamp(160, 80)).toBe(true);
  });

  it("tolerates a sub-pixel line height", () => {
    expect(overflowsClamp(80.6, 80)).toBe(false);
  });

  it("stays silent before the element has been laid out", () => {
    expect(overflowsClamp(0, 0)).toBe(false);
  });
});

describe("CLAMP_LINES", () => {
  // The clamp itself is a stylesheet rule, so the number is checked where it is applied rather
  // than duplicated into a component as an inline style.
  it("is the number of lines the stylesheet keeps", () => {
    const stylesheet = readFileSync(
      fileURLToPath(new URL("../styles.css", import.meta.url)),
      "utf8",
    );

    expect(stylesheet).toContain(`-webkit-line-clamp: ${CLAMP_LINES};`);
  });
});
