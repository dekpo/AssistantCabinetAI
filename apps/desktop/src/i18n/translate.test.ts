import { describe, expect, it } from "vitest";
import { catalogueKeys, createTranslator, interpolate, lookupKey } from "./translate";

const reference = {
  actions: { save: "Save" },
  errors: { work_folder_not_found: "This folder no longer exists ({path})." },
} as const;

describe("lookupKey", () => {
  it("reads a nested key", () => {
    expect(lookupKey(reference, "actions.save")).toBe("Save");
  });

  it("returns nothing for a key that is not there", () => {
    expect(lookupKey(reference, "actions.missing")).toBeUndefined();
    expect(lookupKey(reference, "actions")).toBeUndefined();
  });
});

describe("interpolate", () => {
  it("replaces a placeholder", () => {
    expect(interpolate("in {path}", { path: "D:\\work" })).toBe("in D:\\work");
  });

  it("leaves an unknown placeholder alone rather than failing", () => {
    expect(interpolate("in {path}", { other: 1 })).toBe("in {path}");
  });
});

describe("createTranslator", () => {
  const translated = { actions: { save: "Enregistrer" } } as const;
  const t = createTranslator(translated, reference);

  it("prefers the chosen language", () => {
    expect(t("actions.save")).toBe("Enregistrer");
  });

  it("shows the reference wording for a key not yet translated", () => {
    expect(t("errors.work_folder_not_found", { path: "D:\\x" })).toBe(
      "This folder no longer exists (D:\\x).",
    );
  });

  it("returns the key when nothing knows it, so a gap is visible", () => {
    expect(t("nowhere.at.all")).toBe("nowhere.at.all");
  });
});

describe("catalogueKeys", () => {
  it("flattens to dotted paths", () => {
    expect(catalogueKeys(reference)).toStrictEqual([
      "actions.save",
      "errors.work_folder_not_found",
    ]);
  });
});
