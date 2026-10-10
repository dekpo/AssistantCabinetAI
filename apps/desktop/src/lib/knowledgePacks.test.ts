import { describe, expect, it } from "vitest";
import { HEALTH_MODULES, isModuleOn, withModule } from "./knowledgePacks";

describe("the health modules of the knowledge base", () => {
  it("offers one choice per country", () => {
    expect(HEALTH_MODULES.map((module) => module.id)).toEqual(["health-fr", "health-ch"]);
  });

  it("reads each module from the list of packs", () => {
    expect(isModuleOn([], "health-fr")).toBe(false);
    expect(isModuleOn(["health-fr"], "health-fr")).toBe(true);
    expect(isModuleOn(["health-fr"], "health-ch")).toBe(false);
  });

  it("lets both countries be on at once, as for a cross-border worker", () => {
    const both = withModule(withModule([], "health-fr", true), "health-ch", true);
    expect(both).toEqual(["health-fr", "health-ch"]);
    expect(isModuleOn(both, "health-fr") && isModuleOn(both, "health-ch")).toBe(true);
  });

  it("switches one module off without touching the other, or any other pack", () => {
    expect(withModule(["health-fr", "health-ch", "legal-fr"], "health-fr", false)).toEqual([
      "health-ch",
      "legal-fr",
    ]);
    expect(withModule(["health-fr"], "health-ch", false)).toEqual(["health-fr"]);
    expect(withModule(["health-fr"], "health-fr", true)).toEqual(["health-fr"]);
  });
});
