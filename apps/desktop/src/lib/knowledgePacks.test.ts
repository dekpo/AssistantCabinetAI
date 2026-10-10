import { describe, expect, it } from "vitest";
import { healthModuleOf, packsForHealthModule } from "./knowledgePacks";

describe("the health module of the knowledge base", () => {
  it("reads the module from the list of packs", () => {
    expect(healthModuleOf([])).toBe("none");
    expect(healthModuleOf(["legal-fr"])).toBe("none");
    expect(healthModuleOf(["health-fr"])).toBe("france");
    expect(healthModuleOf(["health-ch"])).toBe("switzerland");
    // A hand-edited file that lists both: Switzerland decides.
    expect(healthModuleOf(["health-fr", "health-ch"])).toBe("switzerland");
  });

  it("writes the list for each choice, one health pack, never two", () => {
    expect(packsForHealthModule([], "france")).toEqual(["health-fr"]);
    expect(packsForHealthModule(["health-fr"], "switzerland")).toEqual(["health-ch"]);
    expect(packsForHealthModule(["health-fr", "health-ch"], "none")).toEqual([]);
    expect(packsForHealthModule(["health-ch"], "france")).toEqual(["health-fr"]);
  });

  it("carries any other pack through untouched", () => {
    expect(packsForHealthModule(["health-fr", "legal-fr"], "switzerland")).toEqual([
      "health-ch",
      "legal-fr",
    ]);
    expect(packsForHealthModule(["legal-fr", "accounting-fr"], "none")).toEqual(["legal-fr", "accounting-fr"]);
  });
});
