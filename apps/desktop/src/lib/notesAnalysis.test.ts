import { describe, expect, it } from "vitest";
import type { FileKind, FileRecord, ProcessingStatus } from "./ipc";
import { needsAnalysisConfirmation, pendingOtherDocuments } from "./notesAnalysis";

function file(
  relativePath: string,
  processingStatus: ProcessingStatus,
  kind: FileKind = "document_text",
): FileRecord {
  return {
    id: relativePath,
    relativePath,
    name: relativePath.split("/").pop() ?? relativePath,
    stem: relativePath,
    extension: "md",
    kind,
    mimeType: "text/markdown",
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

const NOTE = "Notes/2026-10-18_14-32-05-visit.md";

describe("pendingOtherDocuments", () => {
  it("does not count the notes just saved, listed or not", () => {
    expect(pendingOtherDocuments([file(NOTE, "discovered")], [NOTE])).toBe(0);
    expect(pendingOtherDocuments([], [NOTE])).toBe(0);
  });

  it("counts the other documents the pass would still read", () => {
    const files = [
      file(NOTE, "discovered"),
      file("a.pdf", "pending", "document_pdf"),
      file("b.docx", "discovered", "document_office"),
      file("c.pdf", "indexed", "document_pdf"),
      file("d.pdf", "failed", "document_pdf"),
    ];
    expect(pendingOtherDocuments(files, [NOTE])).toBe(2);
  });

  it("ignores what the document pass never reads", () => {
    const files = [file("table.csv", "discovered", "tabular_candidate"), file("x.key", "discovered", "unsupported")];
    expect(pendingOtherDocuments(files, [NOTE])).toBe(0);
  });

  it("compares paths without regard to case", () => {
    expect(pendingOtherDocuments([file("notes/2026-10-18_14-32-05-visit.md", "discovered")], [NOTE])).toBe(0);
  });
});

describe("needsAnalysisConfirmation", () => {
  it("asks only when other documents are also pending", () => {
    expect(needsAnalysisConfirmation([file(NOTE, "discovered")], [NOTE])).toBe(false);
    expect(
      needsAnalysisConfirmation([file(NOTE, "discovered"), file("a.pdf", "pending", "document_pdf")], [NOTE]),
    ).toBe(true);
  });
});
