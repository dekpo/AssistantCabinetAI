import { describe, expect, it } from "vitest";
import { homeRoot, indexKey, isForeignKey, parseIndexKey } from "./indexKey";

describe("the key of a file in the index", () => {
  it("keeps the plain path for a file in the folder of its own family", () => {
    expect(indexKey("documents", "documents", "Courriers/a.pdf")).toBe("Courriers/a.pdf");
    expect(indexKey("data", "data", "factures.xlsx")).toBe("factures.xlsx");
    expect(homeRoot("documents")).toBe("documents");
    expect(homeRoot("data")).toBe("data");
  });

  it("names the folder for a file of the other family", () => {
    expect(indexKey("documents", "data", "Factures/2025.xlsx")).toBe("/documents/Factures/2025.xlsx");
    expect(indexKey("data", "documents", "Courriers/a.pdf")).toBe("/data/Courriers/a.pdf");
  });

  it("gives two keys to the same path in the two folders", () => {
    expect(indexKey("data", "data", "x.csv")).not.toBe(indexKey("documents", "data", "x.csv"));
  });

  it("parses back to the root and the path", () => {
    for (const [root, family, path] of [
      ["documents", "documents", "a/b c.pdf"],
      ["data", "documents", "a/b c.pdf"],
      ["documents", "data", "t.xlsx"],
      ["data", "data", "t.xlsx"],
    ] as const) {
      expect(parseIndexKey(family, indexKey(root, family, path))).toEqual({ root, path });
    }
  });

  it("refuses a key that names no folder, no path or the family's own folder", () => {
    expect(parseIndexKey("documents", "/etc/passwd")).toBeNull();
    expect(parseIndexKey("documents", "/data")).toBeNull();
    expect(parseIndexKey("documents", "/data/")).toBeNull();
    expect(parseIndexKey("documents", "/documents/a.pdf")).toBeNull();
    expect(parseIndexKey("data", "/data/a.csv")).toBeNull();
  });

  it("never takes a name with a colon for a foreign file", () => {
    expect(parseIndexKey("data", "data:x.xlsx")).toEqual({ root: "data", path: "data:x.xlsx" });
    expect(isForeignKey("data:x.xlsx")).toBe(false);
    expect(isForeignKey("/data/x.pdf")).toBe(true);
  });
});
