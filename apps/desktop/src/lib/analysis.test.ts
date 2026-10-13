import { describe, expect, it } from "vitest";
import {
  analysisFraction,
  analysisPending,
  analysisPendingDocuments,
  countPending,
  countPendingDocuments,
} from "./analysis";
import type { FileRecord, IndexProgress, ProcessingStatus } from "./ipc";

function progress(
  processedFiles: number,
  totalFiles: number,
  batchIndex = 0,
  batchTotal = 0,
): IndexProgress {
  return { processedFiles, totalFiles, batchIndex, batchTotal };
}

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

  it("never disagrees with the button beside it", () => {
    const folders = [
      [file("indexed")],
      [file("discovered")],
      [file("failed"), file("pending")],
      [],
    ];

    for (const files of folders) {
      expect(countPending(files) > 0).toBe(analysisPending(files));
    }
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
      fileOfKind("a.odt", "unsupported", "discovered"),
      fileOfKind("b.vtt", "unsupported", "discovered"),
      fileOfKind("c.xlsx", "tabular_candidate", "discovered"),
      fileOfKind("d.png", "image", "failed"),
      fileOfKind("e.txt", "document_text", "indexed"),
    ];

    expect(countPendingDocuments(files)).toBe(0);
    expect(analysisPendingDocuments(files)).toBe(false);
  });

  it("still counts every document kind the pass reads", () => {
    const files = [
      fileOfKind("a.txt", "document_text", "discovered"),
      fileOfKind("b.pdf", "document_pdf", "pending"),
      fileOfKind("c.docx", "document_office", "discovered"),
      fileOfKind("d.jpg", "image", "pending"),
      fileOfKind("e.odt", "unsupported", "discovered"),
    ];

    expect(countPendingDocuments(files)).toBe(4);
    expect(analysisPendingDocuments(files)).toBe(true);
  });

  it("leaves a folder of workbooks to the Data card's own rule", () => {
    const workbooks = [fileOfKind("t.csv", "tabular_candidate", "discovered")];

    expect(countPending(workbooks)).toBe(1);
    expect(countPendingDocuments(workbooks)).toBe(0);
  });
});

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
    expect(analysisFraction(progress(3, 12))).toBe(0.25);
  });

  it("calls a pass over nothing finished rather than dividing by zero", () => {
    expect(analysisFraction(progress(0, 0))).toBe(1);
  });

  it("moves inside a long document instead of standing still", () => {
    // One file of three, two batches of four done: a sixth of the way through the pass.
    expect(analysisFraction(progress(1, 3, 2, 4))).toBeCloseTo(0.5, 5);
    expect(analysisFraction(progress(0, 1, 1, 4))).toBe(0.25);
  });

  it("only goes forward as batches complete", () => {
    const steps = [progress(0, 2, 0, 3), progress(0, 2, 1, 3), progress(0, 2, 3, 3), progress(1, 2)];
    const fractions = steps.map(analysisFraction);

    expect([...fractions].sort((a, b) => a - b)).toStrictEqual(fractions);
  });

  it("never lets a file claim more than its own share", () => {
    expect(analysisFraction(progress(0, 4, 9, 3))).toBe(0.25);
  });

  it("never draws past either end of the bar", () => {
    expect(analysisFraction(progress(20, 12))).toBe(1);
    expect(analysisFraction(progress(-1, 12))).toBe(0);
  });
});
