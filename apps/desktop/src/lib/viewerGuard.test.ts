import { describe, expect, it } from "vitest";
import type { FileRecord } from "./ipc";
import { outsideChange, tabsGone } from "./viewerGuard";
import { tabId } from "./viewerTabs";
import type { OpenFile, ViewerFolder } from "./workspaceView";

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

describe("tabsGone", () => {
  const tab = (folder: ViewerFolder, overrides: Partial<FileRecord>): OpenFile => {
    const file = record(overrides);
    return { id: tabId(folder, file.relativePath), folder, record: file };
  };
  const a = tab("documents", { relativePath: "a.pdf" });
  const b = tab("documents", { relativePath: "b.pdf" });
  const c = tab("data", { relativePath: "c.xlsx" });

  it("reports nothing when every tab is as it was opened", () => {
    expect(
      tabsGone([a, b, c], { documents: [a.record, b.record], data: [c.record] }),
    ).toEqual([]);
  });

  it("reports each tab that changed, with its reason, and leaves the others alone", () => {
    const gone = tabsGone([a, b, c], {
      documents: [record({ relativePath: "a.pdf", processingStatus: "pending" })],
      data: [{ ...c.record, sha256: "other" }],
    });
    expect(gone).toEqual([
      { id: a.id, reason: "not_analysed" },
      { id: b.id, reason: "missing" },
      { id: c.id, reason: "modified" },
    ]);
  });

  it("keeps a tab that is still fine while another is gone", () => {
    expect(tabsGone([a, b], { documents: [a.record] })).toEqual([{ id: b.id, reason: "missing" }]);
  });

  it("compares a tab only with the inventory of its own folder", () => {
    const sameName = tab("data", { relativePath: "a.pdf" });
    expect(tabsGone([a, sameName], { documents: [a.record], data: [] })).toEqual([
      { id: sameName.id, reason: "missing" },
    ]);
  });

  it("leaves the tabs of a folder alone when its inventory could not be read", () => {
    expect(tabsGone([a, c], { documents: [] })).toEqual([{ id: a.id, reason: "missing" }]);
    expect(tabsGone([a, c], {})).toEqual([]);
  });
});
