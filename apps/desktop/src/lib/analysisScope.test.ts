import { describe, expect, it } from "vitest";
import { scopedPaths, toggleScopeFile, wholeFolderScope } from "./analysisScope";
import type { FileRecord } from "./ipc";

function file(relativePath: string, id: string): FileRecord {
  return { id, relativePath } as FileRecord;
}

describe("analysis scope", () => {
  it("starts as the whole folder", () => {
    const scope = wholeFolderScope(10);

    expect(scope.mode).toStrictEqual({ kind: "whole_folder" });
    expect(scopedPaths(scope)).toStrictEqual([]);
  });

  it("pins the id of a file when it is ticked", () => {
    const scope = toggleScopeFile(wholeFolderScope(10), file("a.pdf", "id-a"), 20);

    expect(scope.mode).toStrictEqual({
      kind: "explicit",
      entries: [{ relativePath: "a.pdf", pinnedId: "id-a", addedAt: 20 }],
    });
    expect(scope.createdAt).toBe(10);
    expect(scope.updatedAt).toBe(20);
  });

  it("keeps the other files when one is unticked", () => {
    let scope = toggleScopeFile(wholeFolderScope(10), file("a.pdf", "id-a"), 20);
    scope = toggleScopeFile(scope, file("b.pdf", "id-b"), 30);
    scope = toggleScopeFile(scope, file("a.pdf", "id-a"), 40);

    expect(scopedPaths(scope)).toStrictEqual(["b.pdf"]);
  });

  it("returns to the whole folder when the last file is unticked", () => {
    let scope = toggleScopeFile(wholeFolderScope(10), file("a.pdf", "id-a"), 20);
    scope = toggleScopeFile(scope, file("a.pdf", "id-a"), 30);

    expect(scope.mode).toStrictEqual({ kind: "whole_folder" });
    expect(scope.createdAt).toBe(10);
  });
});
