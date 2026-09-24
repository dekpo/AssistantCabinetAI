import { describe, expect, it } from "vitest";
import { analysisFraction, analysisPending } from "./analysis";
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

describe("analysisPending", () => {
  it("is true for a folder nothing has read yet", () => {
    expect(analysisPending([file("discovered"), file("discovered")])).toBe(true);
  });

  it("is true when one file changed since the last pass", () => {
    expect(analysisPending([file("indexed"), file("pending")])).toBe(true);
  });

  it("is false once every file has been read", () => {
    expect(analysisPending([file("indexed"), file("indexed")])).toBe(false);
  });

  it("does not stay true for a file that cannot be read at all", () => {
    // Analysing again would fail again, so the button must stop asking for it.
    expect(analysisPending([file("indexed"), file("failed")])).toBe(false);
  });

  it("is false for an empty folder", () => {
    expect(analysisPending([])).toBe(false);
  });
});

describe("analysisFraction", () => {
  it("reports how far the pass has got", () => {
    expect(analysisFraction({ processedFiles: 3, totalFiles: 12 })).toBe(0.25);
  });

  it("calls a pass over nothing finished rather than dividing by zero", () => {
    expect(analysisFraction({ processedFiles: 0, totalFiles: 0 })).toBe(1);
  });

  it("never draws past either end of the bar", () => {
    expect(analysisFraction({ processedFiles: 20, totalFiles: 12 })).toBe(1);
    expect(analysisFraction({ processedFiles: -1, totalFiles: 12 })).toBe(0);
  });
});
