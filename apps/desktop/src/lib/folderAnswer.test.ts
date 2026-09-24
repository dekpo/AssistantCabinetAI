import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { formatFolderAnswer } from "./folderAnswer";
import type { FileRecord, FolderAnswer } from "./ipc";

/**
 * The sentence for a deterministic answer is written here, so these tests are where the
 * language contract meets the Work Folder inventory: machine codes in, the practice's own
 * language out, and never a file the inventory did not report.
 */

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function record(relativePath: string, overrides: Partial<FileRecord> = {}): FileRecord {
  const name = relativePath.split("/").pop() ?? relativePath;
  const dot = name.lastIndexOf(".");
  return {
    id: `id-${relativePath}`,
    relativePath,
    name,
    stem: dot > 0 ? name.slice(0, dot) : name,
    extension: dot > 0 ? name.slice(dot + 1).toLowerCase() : "",
    kind: "document_pdf",
    mimeType: "application/pdf",
    sizeBytes: 10,
    modifiedAt: null,
    sha256: null,
    readability: "readable",
    processingStatus: "indexed",
    extractionMethod: "native_text",
    indexed: true,
    indexMetadata: null,
    ...overrides,
  };
}

describe("a counting answer", () => {
  it("says how many files there are, in either language", () => {
    const answer: FolderAnswer = { kind: "file_count", total: 15 };

    expect(formatFolderAnswer(english, answer)).toBe("Your documents folder holds 15 files.");
    expect(formatFolderAnswer(french, answer)).toBe(
      "Votre dossier de documents contient 15 fichiers.",
    );
  });

  it("uses the singular for one file", () => {
    expect(formatFolderAnswer(english, { kind: "file_count", total: 1 })).toBe(
      "Your documents folder holds 1 file.",
    );
  });

  it("keeps the extension with its dot", () => {
    const answer: FolderAnswer = { kind: "extension_count", extension: "pdf", count: 5 };

    expect(formatFolderAnswer(english, answer)).toContain(".pdf");
    expect(formatFolderAnswer(english, answer)).toContain("5 files");
  });
});

describe("a listing answer", () => {
  it("prints every relative path the inventory gave and nothing else", () => {
    const files = [record("2026/mars/neurologie.pdf"), record("administratif/assurance.txt")];

    const text = formatFolderAnswer(english, { kind: "file_list", files });

    expect(text).toContain("- 2026/mars/neurologie.pdf");
    expect(text).toContain("- administratif/assurance.txt");
    expect(text.split("\n").filter((line) => line.startsWith("- "))).toHaveLength(2);
  });

  it("says what happened to each file, not only what it is called", () => {
    // Whether anything could be read from a file is what decides whether it can be asked about,
    // so a list of bare names leaves out the fact that matters most.
    const files = [
      record("rapport.pdf"),
      record("scan.png", {
        readability: "unreadable",
        processingStatus: "failed",
        extractionMethod: "none",
        indexed: false,
      }),
      record("nouveau.txt", { processingStatus: "discovered", indexed: false }),
    ];

    const text = formatFolderAnswer(english, { kind: "file_list", files });

    expect(text).toContain("- rapport.pdf — analyzed");
    expect(text).toContain("- scan.png — could not be read");
    expect(text).toContain("- nouveau.txt — not analyzed yet");
  });

  it("uses the same words in French", () => {
    const text = formatFolderAnswer(french, {
      kind: "file_list",
      files: [record("rapport.pdf")],
    });

    expect(text).toContain("- rapport.pdf — analysé");
  });

  it("keeps the folder structure readable by leaving its indentation alone", () => {
    const text = formatFolderAnswer(english, {
      kind: "folder_tree",
      root: "AssistantCabinetAI",
      lines: ["2026/", "  mars/", "    neurologie.pdf"],
    });

    expect(text).toContain("```");
    expect(text).toContain("    neurologie.pdf");
  });

  it("says so rather than printing an empty list when there is no subfolder", () => {
    expect(formatFolderAnswer(english, { kind: "folder_list", folders: [] })).toBe(
      "Your documents folder has no subfolder.",
    );
  });
});

describe("an answer that must not resolve itself", () => {
  it("shows every candidate and picks none of them", () => {
    const text = formatFolderAnswer(french, {
      kind: "ambiguous_reference",
      query: "neurologie.pdf",
      candidates: [record("2026/janvier/neurologie.pdf"), record("2026/mars/neurologie.pdf")],
    });

    expect(text).toContain("neurologie.pdf");
    expect(text).toContain("- 2026/janvier/neurologie.pdf");
    expect(text).toContain("- 2026/mars/neurologie.pdf");
    expect(text).toMatch(/Lequel/);
  });

  it("says a file is not there rather than answering about another one", () => {
    const text = formatFolderAnswer(english, {
      kind: "no_matching_file",
      query: "secret-report.pdf",
    });

    expect(text).toBe("No file named secret-report.pdf is in your documents folder.");
  });

  it("says an unreadable file has nothing to quote", () => {
    const text = formatFolderAnswer(english, {
      kind: "file_unreadable",
      file: record("patient-report.png", {
        kind: "image",
        readability: "unreadable",
        processingStatus: "failed",
        extractionMethod: "none",
        indexed: false,
      }),
    });

    expect(text).toContain("patient-report.png");
    expect(text).toContain("nothing to quote");
  });
});

describe("a file's details", () => {
  it("names how the text was obtained, keeping recognition apart from native text", () => {
    const recognised = formatFolderAnswer(english, {
      kind: "file_details",
      file: record("scan.pdf", { extractionMethod: "recognised_ocr" }),
    });
    const native = formatFolderAnswer(english, {
      kind: "file_details",
      file: record("letter.pdf"),
    });

    expect(recognised).toContain("recognized from an image");
    expect(native).toContain("taken from the file itself");
    expect(recognised).not.toBe(native);
  });

  it("reports the extension the filesystem gave, not one the content claims", () => {
    const text = formatFolderAnswer(english, {
      kind: "file_details",
      file: record("report.txt", { kind: "document_text" }),
    });

    expect(text).toContain(".txt");
    expect(text).not.toContain(".pdf");
  });
});

describe("every answer has a sentence", () => {
  const files = [record("a.pdf")];
  const every: FolderAnswer[] = [
    { kind: "file_count", total: 1 },
    { kind: "file_list", files },
    { kind: "extension_count", extension: "pdf", count: 1 },
    { kind: "extension_list", extension: "pdf", files },
    { kind: "image_list", files },
    { kind: "unreadable_count", count: 1 },
    { kind: "unreadable_list", files },
    { kind: "indexed_count", count: 1 },
    { kind: "indexed_list", files },
    { kind: "folder_list", folders: ["2026"] },
    { kind: "folder_tree", root: "cabinet", lines: ["a.pdf"] },
    { kind: "file_details", file: files[0] as FileRecord },
    { kind: "ambiguous_reference", query: "a.pdf", candidates: files },
    { kind: "no_matching_file", query: "b.pdf" },
    { kind: "file_unreadable", file: files[0] as FileRecord },
  ];

  it.each(every)("$kind reads as a sentence in both languages", (answer) => {
    for (const t of [english, french]) {
      const text = formatFolderAnswer(t, answer);
      expect(text.trim()).not.toBe("");
      // A missing catalogue entry would show the key itself.
      expect(text).not.toContain("folderAnswer.");
    }
  });
});
