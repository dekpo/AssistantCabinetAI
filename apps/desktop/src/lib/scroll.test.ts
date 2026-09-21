import { describe, expect, it } from "vitest";
import { BOTTOM_TOLERANCE_PX, isNearBottom, scrollBehaviour } from "./scroll";

describe("isNearBottom", () => {
  it("is at the end when the conversation is shorter than the panel", () => {
    expect(isNearBottom(400, 0, 400)).toBe(true);
  });

  it("is at the end when the container is scrolled all the way down", () => {
    expect(isNearBottom(2000, 1600, 400)).toBe(true);
  });

  it("forgives the last few pixels of rounding", () => {
    expect(isNearBottom(2000, 1600 - BOTTOM_TOLERANCE_PX, 400)).toBe(true);
  });

  it("is not at the end once she has read her way up", () => {
    expect(isNearBottom(2000, 900, 400)).toBe(false);
  });
});

describe("scrollBehaviour", () => {
  it("glides by default", () => {
    expect(scrollBehaviour(false)).toBe("smooth");
  });

  it("jumps when the machine asks for less motion", () => {
    expect(scrollBehaviour(true)).toBe("auto");
  });
});
