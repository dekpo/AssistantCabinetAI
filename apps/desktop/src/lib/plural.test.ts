import { describe, expect, it } from "vitest";
import { counted, pluralise } from "./plural";

describe("pluralise", () => {
  it("uses the singular for zero and one", () => {
    expect(pluralise(0, "fichier indexé", "fichiers indexés")).toBe("fichier indexé");
    expect(pluralise(1, "fichier indexé", "fichiers indexés")).toBe("fichier indexé");
  });

  it("uses the plural from two", () => {
    expect(pluralise(2, "fichier indexé", "fichiers indexés")).toBe("fichiers indexés");
    expect(pluralise(17, "inchangé", "inchangés")).toBe("inchangés");
  });
});

describe("counted", () => {
  it("matches the index summary shape", () => {
    expect(counted(0, "fichier indexé", "fichiers indexés")).toBe("0 fichier indexé");
    expect(counted(17, "inchangé", "inchangés")).toBe("17 inchangés");
  });
});
