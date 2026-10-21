import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import * as analysisScope from "./analysisScope";
import { foundPaths, foundReasons } from "./finderField";
import type { FinderResult } from "./ipc";

describe("a search never selects", () => {
  const read = (relative: string) =>
    readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

  it("has no selection callback in the hook or the state module", () => {
    for (const source of ["../state/useFinderSearch.ts", "./finderSearchState.ts"]) {
      expect(read(source), source).not.toMatch(/onSelect|scopeFromSearch/);
    }
  });

  it("no longer offers a function that replaces the selection from a search", () => {
    expect(Object.keys(analysisScope)).not.toContain("scopeFromSearch");
  });
});

describe("the files of a result", () => {
  it("are read in the result's order", () => {
    const result = {
      files: [{ relativePath: "b.pdf" }, { relativePath: "a.pdf" }],
    } as FinderResult;

    expect(foundPaths(result)).toStrictEqual(["b.pdf", "a.pdf"]);
  });
});

describe("the reasons of a result", () => {
  it("are read by path, as the finder reported them", () => {
    const result = {
      files: [
        { relativePath: "b.pdf", reasons: [{ kind: "word", detail: "passage" }] },
        { relativePath: "a.pdf", reasons: [] },
      ],
    } as unknown as FinderResult;

    expect(foundReasons(result)).toStrictEqual({
      "b.pdf": [{ kind: "word", detail: "passage" }],
      "a.pdf": [],
    });
  });
});
