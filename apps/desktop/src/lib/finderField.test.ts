import { describe, expect, it } from "vitest";
import { noDocumentsScope, scopeFromSearch, selectionOf, tickedPaths, wholeFolderScope } from "./analysisScope";
import { foundNotAnalysed, foundPaths, selectedBySearch, shownFiles } from "./finderField";
import type { FileRecord, FinderResult } from "./ipc";

function file(relativePath: string, id: string): FileRecord {
  return { id, relativePath } as FileRecord;
}

const A = file("2026-03_courrier-a.pdf", "id-a");
const B = file("2026-04_courrier-b.pdf", "id-b");
const C = file("2026-05_facture-c.pdf", "id-c");
/** Found by name only: never analysed, so it can be found and cannot be selected. */
const D = file("2026-06_courrier-d.pdf", "id-d");
const FOLDER = [A, B, C, D];
const ANALYSED = [A, B, C];

describe("the selection a search makes", () => {
  it("replaces the selection with the analysed files it found, each pinned to its content", () => {
    const before = scopeFromSearch(noDocumentsScope(10), [C.relativePath], ANALYSED, 15);
    const scope = scopeFromSearch(before, [A.relativePath, B.relativePath], ANALYSED, 20);

    expect(scope.mode).toStrictEqual({
      kind: "explicit",
      entries: [
        { relativePath: A.relativePath, pinnedId: "id-a", addedAt: 20 },
        { relativePath: B.relativePath, pinnedId: "id-b", addedAt: 20 },
      ],
    });
    expect(scope.createdAt).toBe(10);
    expect(scope.updatedAt).toBe(20);
  });

  it("never selects a file that is not analysed, and says how many it found without selecting", () => {
    const found = [A.relativePath, D.relativePath];

    const scope = scopeFromSearch(noDocumentsScope(10), found, ANALYSED, 20);

    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([A.relativePath]);
    expect(selectedBySearch(found, ANALYSED)).toBe(1);
    expect(foundNotAnalysed(found, ANALYSED)).toBe(1);
  });

  it("selects nothing when nothing is found, or when only unanalysed files are", () => {
    const start = scopeFromSearch(noDocumentsScope(10), [A.relativePath], ANALYSED, 15);

    expect(selectionOf(scopeFromSearch(start, [], ANALYSED, 20))).toBe("none");
    expect(selectionOf(scopeFromSearch(start, [D.relativePath], ANALYSED, 20))).toBe("none");
  });

  it("is the whole folder when every analysed file is found, as ticking the last one is", () => {
    const scope = scopeFromSearch(
      noDocumentsScope(10),
      [A.relativePath, B.relativePath, C.relativePath, D.relativePath],
      ANALYSED,
      20,
    );

    expect(selectionOf(scope)).toBe("all");
  });

  it("starts from the whole folder like any other change: the search narrows it", () => {
    const scope = scopeFromSearch(wholeFolderScope(10), [B.relativePath], ANALYSED, 20);

    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([B.relativePath]);
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
    const scope = scopeFromSearch(noDocumentsScope(10), found, ANALYSED, 20);

    expect(shownFiles(FOLDER, found, true)).toStrictEqual(FOLDER);
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([A.relativePath]);
  });

  it("keeps the selection when the search is cleared: the listing is the only thing that changes", () => {
    const scope = scopeFromSearch(noDocumentsScope(10), found, ANALYSED, 20);

    // Clearing the field is `found = null`: the listing is whole again, the scope is the same value.
    expect(shownFiles(FOLDER, null, false)).toStrictEqual(FOLDER);
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([A.relativePath]);
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
