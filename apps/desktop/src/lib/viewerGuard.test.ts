import { describe, expect, it } from "vitest";
import type { FileRecord } from "./ipc";
import { outsideChange } from "./viewerGuard";

function record(overrides: Partial<FileRecord>): FileRecord {
  return {
    id: "id",
    relativePath: "a/b.pdf",
    name: "b.pdf",
    stem: "b",
    extension: "pdf",
    kind: "document",
    mimeType: "application/pdf",
    sizeBytes: 1,
    modifiedAt: null,
    sha256: "hash-1",
    readability: "readable",
    processingStatus: "indexed",
    extractionMethod: "text",
    indexed: true,
    indexMetadata: null,
    ...overrides,
  } as FileRecord;
}

describe("outsideChange", () => {
  const open = record({});

  it("sees no change when the file is as it was opened", () => {
    expect(outsideChange(open, [record({ relativePath: "other.pdf" }), record({})])).toBeNull();
  });

  it("says missing when the path is gone from the inventory (renamed, moved or deleted)", () => {
    expect(outsideChange(open, [record({ relativePath: "a/renamed.pdf" })])).toBe("missing");
    expect(outsideChange(open, [])).toBe("missing");
  });

  it("says not analysed when the file is no longer indexed", () => {
    expect(outsideChange(open, [record({ processingStatus: "pending" })])).toBe("not_analysed");
    expect(outsideChange(open, [record({ processingStatus: "failed" })])).toBe("not_analysed");
  });

  it("says modified when the content hash differs", () => {
    expect(outsideChange(open, [record({ sha256: "hash-2" })])).toBe("modified");
  });

  it("checks the status before the hash", () => {
    expect(outsideChange(open, [record({ sha256: "hash-2", processingStatus: "pending" })])).toBe(
      "not_analysed",
    );
  });
});
