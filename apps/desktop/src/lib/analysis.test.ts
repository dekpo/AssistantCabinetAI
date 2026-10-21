import { describe, expect, it } from "vitest";
import { countPending, countPendingDocuments, unfinishedPercent } from "./analysis";
import type { FileRecord, ProcessingStatus } from "./ipc";

function file(processingStatus: ProcessingStatus): FileRecord {
  return {
    id: processingStatus,
    relativePath: `${processingStatus}.pdf`,
    name: `${processingStatus}.pdf`,
    stem: processingStatus,
    extension: "pdf",
    kind: "document_pdf",
    mimeType: "application/pdf",
    sizeBytes: 1,
    modifiedAt: null,
    sha256: null,
    readability: "not_assessed",
    processingStatus,
    extractionMethod: "none",
    indexed: processingStatus === "indexed",
    indexMetadata: null,
  };
}

describe("countPending", () => {
  it("counts the files one more pass would change", () => {
    expect(countPending([file("discovered"), file("pending"), file("indexed")])).toBe(2);
  });

  it("leaves out a file that was read and produced nothing", () => {
    // Counting it would put a number on the button that analysing can never bring down.
    expect(countPending([file("failed"), file("indexed")])).toBe(0);
  });

  it("is zero for a folder with nothing in it", () => {
    expect(countPending([])).toBe(0);
  });
});

function fileOfKind(
  name: string,
  kind: FileRecord["kind"],
  processingStatus: ProcessingStatus,
): FileRecord {
  return {
    ...file(processingStatus),
    id: name,
    relativePath: name,
    name,
    stem: name.split(".")[0] ?? name,
    extension: name.split(".")[1] ?? "",
    kind,
    indexed: processingStatus === "indexed",
  };
}

describe("countPendingDocuments", () => {
  it("leaves the button inactive when only files the pass cannot read are left", () => {
    // The pass never writes a row for these, so they stay "discovered" for ever.
    const files = [
      fileOfKind("a.pages", "unsupported", "discovered"),
      fileOfKind("b.key", "unsupported", "discovered"),
      fileOfKind("c.xlsx", "tabular_candidate", "discovered"),
      fileOfKind("d.png", "image", "failed"),
      fileOfKind("e.txt", "document_text", "indexed"),
    ];

    expect(countPendingDocuments(files)).toBe(0);
  });

  it("still counts every document kind the pass reads", () => {
    const files = [
      fileOfKind("a.txt", "document_text", "discovered"),
      fileOfKind("b.pdf", "document_pdf", "pending"),
      fileOfKind("c.docx", "document_office", "discovered"),
      fileOfKind("d.jpg", "image", "pending"),
      fileOfKind("e.pages", "unsupported", "discovered"),
    ];

    expect(countPendingDocuments(files)).toBe(4);
  });

  it("leaves a folder of workbooks to the tabular pass's own rule", () => {
    const workbooks = [fileOfKind("t.csv", "tabular_candidate", "discovered")];

    expect(countPending(workbooks)).toBe(1);
    expect(countPendingDocuments(workbooks)).toBe(0);
  });
});

describe("the share shown for the meaning search", () => {
  it("shows a whole percent only while the meaning search is incomplete", () => {
    expect(unfinishedPercent({ done: 62, total: 100 })).toBe(62);
    expect(unfinishedPercent({ done: 99, total: 100 })).toBe(99);
  });

  it("shows nothing at 100 %, when nothing is known, or when there is nothing to compute", () => {
    expect(unfinishedPercent({ done: 100, total: 100 })).toBeNull();
    expect(unfinishedPercent(null)).toBeNull();
    expect(unfinishedPercent({ done: 0, total: 0 })).toBeNull();
  });
});
