import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import * as analysisScope from "./analysisScope";
import {
  addScopeFiles,
  noDocumentsScope,
  selectionOf,
  tickedPaths,
  wholeFolderScope,
} from "./analysisScope";
import { foundAnalysed, foundNotAnalysed, foundPaths, resultsToTick, shownFiles } from "./finderField";
import type { FileRecord, FinderResult } from "./ipc";

function file(relativePath: string, id: string): FileRecord {
  return { id, relativePath } as FileRecord;
}

const A = file("2026-03_courrier-a.pdf", "id-a");
const B = file("2026-04_courrier-b.pdf", "id-b");
const C = file("2026-05_facture-c.pdf", "id-c");
/** Found by name only: never analysed, so it can be found and cannot be ticked. */
const D = file("2026-06_courrier-d.pdf", "id-d");
const FOLDER = [A, B, C, D];
const ANALYSED = [A, B, C];

describe("the results a search offers to tick", () => {
  it("are the analysed files found that are not selected yet", () => {
    const found = [A.relativePath, B.relativePath, D.relativePath];
    const selected = addScopeFiles(noDocumentsScope(10), [A], ANALYSED, 15);

    expect(resultsToTick(found, ANALYSED, noDocumentsScope(10))).toStrictEqual([A, B]);
    expect(resultsToTick(found, ANALYSED, selected)).toStrictEqual([B]);
  });

  it("are none when everything found is ticked, or when only unanalysed files were found", () => {
    const selected = addScopeFiles(noDocumentsScope(10), [A, B], ANALYSED, 15);

    expect(resultsToTick([A.relativePath, B.relativePath], ANALYSED, selected)).toStrictEqual([]);
    expect(resultsToTick([D.relativePath], ANALYSED, noDocumentsScope(10))).toStrictEqual([]);
    expect(resultsToTick([], ANALYSED, noDocumentsScope(10))).toStrictEqual([]);
  });

  it("are none when the selection is already the whole folder", () => {
    expect(resultsToTick([A.relativePath], ANALYSED, wholeFolderScope(10))).toStrictEqual([]);
  });

  it("are added to the selection without removing what was ticked, each pinned to its content", () => {
    const before = addScopeFiles(noDocumentsScope(10), [C], ANALYSED, 15);
    const found = [A.relativePath, B.relativePath, D.relativePath];

    const scope = addScopeFiles(before, resultsToTick(found, ANALYSED, before), ANALYSED, 20);

    // A and B were added, C stays, D (not analysed) was never offered: every analysed file is ticked.
    expect(selectionOf(scope)).toBe("all");
    const partial = addScopeFiles(before, [A], ANALYSED, 20);
    expect(partial.mode).toStrictEqual({
      kind: "explicit",
      entries: [
        { relativePath: C.relativePath, pinnedId: "id-c", addedAt: 15 },
        { relativePath: A.relativePath, pinnedId: "id-a", addedAt: 20 },
      ],
    });
  });

  it("leave a selection that is already the whole folder as it is", () => {
    const whole = wholeFolderScope(10);

    expect(addScopeFiles(whole, [A], ANALYSED, 20)).toBe(whole);
  });

  it("say how many files were found and could not be ticked", () => {
    const found = [A.relativePath, D.relativePath];

    expect(foundAnalysed(found, ANALYSED)).toBe(1);
    expect(foundNotAnalysed(found, ANALYSED)).toBe(1);
  });
});

describe("the listing under the field", () => {
  const found = [A.relativePath, D.relativePath];

  it("shows the whole folder when no search is active", () => {
    expect(shownFiles(FOLDER, null, false)).toStrictEqual(FOLDER);
    expect(shownFiles(FOLDER, null, true)).toStrictEqual(FOLDER);
  });

  it("shows only the files found while a search is active, in the folder's order", () => {
    expect(shownFiles(FOLDER, [D.relativePath, A.relativePath], false)).toStrictEqual([A, D]);
  });

  it("shows the whole folder again on request, as a view that changes no selection", () => {
    const scope = addScopeFiles(noDocumentsScope(10), [A], ANALYSED, 20);

    expect(shownFiles(FOLDER, found, true)).toStrictEqual(FOLDER);
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([A.relativePath]);
  });

  it("changes nothing when nothing is found: the listing is empty and the selection is the same value", () => {
    const scope = addScopeFiles(noDocumentsScope(10), [A], ANALYSED, 20);

    expect(shownFiles(FOLDER, [], false)).toStrictEqual([]);
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([A.relativePath]);
  });

  it("keeps the selection when the search is cleared: the listing is the only thing that changes", () => {
    const scope = addScopeFiles(noDocumentsScope(10), [A], ANALYSED, 20);

    // Clearing the field is `found = null`: the listing is whole again, the scope is the same value.
    expect(shownFiles(FOLDER, null, false)).toStrictEqual(FOLDER);
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([A.relativePath]);
  });
});

describe("a search never selects", () => {
  const read = (relative: string) =>
    readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");

  it("has no selection callback in the hook, the state module or the cards", () => {
    for (const source of [
      "../state/useFinderSearch.ts",
      "./finderSearchState.ts",
      "../components/WorkFolderCard.tsx",
      "../components/DataFolderCard.tsx",
    ]) {
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
