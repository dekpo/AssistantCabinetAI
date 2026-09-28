import { describe, expect, it } from "vitest";
import {
  noDocumentsScope,
  scopedPaths,
  selectionOf,
  tickedPaths,
  toggleAll,
  toggleScopeFile,
  wholeFolderScope,
} from "./analysisScope";
import type { FileRecord } from "./ipc";

function file(relativePath: string, id: string): FileRecord {
  return { id, relativePath } as FileRecord;
}

const A = file("a.pdf", "id-a");
const B = file("b.pdf", "id-b");
const C = file("c.pdf", "id-c");
const ANALYSED = [A, B, C];

describe("document selection", () => {
  it("starts with no document", () => {
    const scope = noDocumentsScope(10);

    expect(scope.mode).toStrictEqual({ kind: "explicit", entries: [] });
    expect(selectionOf(scope)).toBe("none");
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([]);
  });

  it("pins the id of a file when it is ticked", () => {
    const scope = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);

    expect(scope.mode).toStrictEqual({
      kind: "explicit",
      entries: [{ relativePath: "a.pdf", pinnedId: "id-a", addedAt: 20 }],
    });
    expect(selectionOf(scope)).toBe("some");
    expect(scope.createdAt).toBe(10);
    expect(scope.updatedAt).toBe(20);
  });

  it("keeps the other files when one is unticked", () => {
    let scope = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);
    scope = toggleScopeFile(scope, B, ANALYSED, 30);
    scope = toggleScopeFile(scope, A, ANALYSED, 40);

    expect(scopedPaths(scope)).toStrictEqual(["b.pdf"]);
  });

  it("returns to no document, never to the whole folder, when the last file is unticked", () => {
    let scope = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);
    scope = toggleScopeFile(scope, A, ANALYSED, 30);

    expect(selectionOf(scope)).toBe("none");
    expect(scope.createdAt).toBe(10);
    expect(scope.updatedAt).toBe(30);
  });

  it("becomes the whole folder once every analysed file is ticked", () => {
    let scope = noDocumentsScope(10);
    for (const each of ANALYSED) {
      scope = toggleScopeFile(scope, each, ANALYSED, 20);
    }

    expect(scope.mode).toStrictEqual({ kind: "whole_folder" });
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual(["a.pdf", "b.pdf", "c.pdf"]);
  });

  it("keeps every other analysed file, freshly pinned, when one is unticked from the whole folder", () => {
    const scope = toggleScopeFile(wholeFolderScope(10), B, ANALYSED, 50);

    expect(scope.mode).toStrictEqual({
      kind: "explicit",
      entries: [
        { relativePath: "a.pdf", pinnedId: "id-a", addedAt: 50 },
        { relativePath: "c.pdf", pinnedId: "id-c", addedAt: 50 },
      ],
    });
  });

  it("selects every document from none or some, and none from all", () => {
    const some = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);

    expect(selectionOf(toggleAll(noDocumentsScope(10), 30))).toBe("all");
    expect(selectionOf(toggleAll(some, 30))).toBe("all");
    expect(selectionOf(toggleAll(wholeFolderScope(10), 30))).toBe("none");
    expect(toggleAll(some, 30).createdAt).toBe(10);
  });

  it("does not call a single ticked file the whole folder when nothing else is analysed yet", () => {
    // With one analysed file, ticking it is ticking every analysed file.
    expect(selectionOf(toggleScopeFile(noDocumentsScope(10), A, [A], 20))).toBe("all");
    // With none analysed there is nothing "every" could mean, so a stray tick stays a choice.
    expect(selectionOf(toggleScopeFile(noDocumentsScope(10), A, [], 20))).toBe("some");
  });
});
