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

describe("a word search of the selected files", () => {
  const listed = [
    {
      relativePath: "caisses/2026-03-26_decompte-helsana.pdf",
      title: null,
      businessType: null,
      excerpt: "Votre *franchise* annuelle de 300 francs est _atteinte_.",
      page: 2,
    },
    {
      relativePath: "2026-04-02_courrier-css.txt",
      title: null,
      businessType: null,
      excerpt: null,
      page: null,
    },
  ];
  const search: FolderAnswer = {
    kind: "document_search",
    domain: "documents",
    query: "franchise annuelle",
    terms: ["franchise", "annuelle"],
    prefix: false,
    quoted: false,
    businessType: null,
    identifier: false,
    entity: null,
    total: 3,
    listed,
    others: ["autres/2026-05-01_note.txt"],
    coverage: { searched: 40, unreadable: 1, unanalysed: 2, partial: 0 },
  };

  it("says what was searched, how many files hold it, and that it is the word itself", () => {
    const text = formatFolderAnswer(french, search);

    expect(text).toContain("Recherche du mot « franchise annuelle » : 3 fichiers");
    expect(text).toContain("Seul le mot lui-même est cherché");
    expect(text).toContain("- caisses/2026-03-26_decompte-helsana.pdf (page 2)");
    expect(text).toContain("- 2026-04-02_courrier-css.txt");
    expect(text).toContain("1 fichier de plus, sans extrait");
  });

  it("states the files it could not search, and never reads a passage as Markdown", () => {
    const text = formatFolderAnswer(french, search);

    expect(text).toContain("cherchés par leur nom seulement : 2 fichiers");
    expect(text).toContain("Sans texte lisible, donc non cherchés : 1 fichier");
    expect(text).toContain("Votre \\*franchise\\* annuelle");
    expect(text).toContain("\\_atteinte\\_");
    expect(text).not.toContain("Votre *franchise*");
  });

  it("names the type of document it was asked for, and says it is a number for an identifier", () => {
    const withType = formatFolderAnswer(french, {
      ...(search as Extract<FolderAnswer, { kind: "document_search" }>),
      businessType: "invoice",
    });
    expect(withType).toContain("Type de document recherché : Facture.");

    const identifier = formatFolderAnswer(english, {
      ...(search as Extract<FolderAnswer, { kind: "document_search" }>),
      query: "756.3052.5778.43",
      identifier: true,
    });
    expect(identifier).toContain("Search for the number “756.3052.5778.43”, in any spelling");
    expect(identifier).not.toContain("Only the word itself is searched");
  });

  it("says when the typed name is one entity of the knowledge base", () => {
    const text = formatFolderAnswer(french, {
      ...(search as Extract<FolderAnswer, { kind: "document_search" }>),
      query: "Mimi Martin",
      entity: "Marie Martin",
    });
    expect(text).toContain("« Mimi Martin » est reconnu comme Marie Martin");
    expect(formatFolderAnswer(french, search)).not.toContain("est reconnu comme");
  });

  it("states the folder of each list when documents and spreadsheets are both searched", () => {
    const documents = search as Extract<FolderAnswer, { kind: "document_search" }>;
    const text = formatFolderAnswer(french, {
      kind: "document_search_both",
      documents,
      workbooks: {
        ...documents,
        domain: "data",
        query: "dermatologie",
        total: 1,
        listed: [
          {
            relativePath: "rendez-vous.csv",
            title: null,
            businessType: null,
            excerpt: null,
            page: null,
          },
        ],
        others: [],
        coverage: { searched: 3, unreadable: 0, unanalysed: 1, partial: 0 },
      },
    });

    expect(text.indexOf("**Documents**")).toBeLessThan(text.indexOf("**Tableurs**"));
    expect(text).toContain("dans les documents sélectionnés");
    expect(text).toContain("Recherche du mot « dermatologie » : 1 fichier dans les tableurs sélectionnés.");
    expect(text).toContain("- rendez-vous.csv");
    expect(text).toContain("cherchés par leur nom seulement : 1 fichier");
  });

  it("says that nothing was found without inventing a file", () => {
    const text = formatFolderAnswer(english, {
      ...(search as Extract<FolderAnswer, { kind: "document_search" }>),
      total: 0,
      listed: [],
      others: [],
      coverage: { searched: 5, unreadable: 0, unanalysed: 0, partial: 0 },
    });
    expect(text).toContain("0 file in the selected documents");
    expect(text).not.toContain("- ");
  });
});

describe("a ranking by date", () => {
  const ranked: FolderAnswer = {
    kind: "ranked_file",
    latest: true,
    noun: "courrier",
    byType: false,
    candidates: 3,
    unranked: 1,
    mixedBasis: true,
    ranking: [
      { relativePath: "2026-10-01_courrier-dermatologie.pdf", date: "2026-10-01", basis: "filename" },
      { relativePath: "courrier-sans-date.txt", date: "2026-06-01", basis: "modified" },
    ],
  };

  it("names the file, the ranking it comes from and where each date comes from", () => {
    const text = formatFolderAnswer(french, ranked);

    expect(text).toContain("Parmi 3 fichiers lisibles dont le nom contient « courrier »");
    expect(text).toContain("le plus récent est le premier de cette liste");
    expect(text).toContain(
      "- 2026-10-01_courrier-dermatologie.pdf — 2026-10-01 (date au début du nom du fichier)",
    );
    expect(text).toContain("- courrier-sans-date.txt — 2026-06-01 (date de modification du fichier)");
    expect(text).toContain("Les dates ne sont pas toutes de même nature");
    expect(text).toContain("1 fichier sans date connue, non classés.");
  });

  it("says when the dates come from the document and when the noun is a type", () => {
    const text = formatFolderAnswer(french, {
      ...(ranked as Extract<FolderAnswer, { kind: "ranked_file" }>),
      noun: "factures",
      byType: true,
      mixedBasis: false,
      unranked: 0,
      ranking: [
        { relativePath: "medsupply-fac-020.txt", date: "2026-06-02", basis: "header_label" },
        { relativePath: "2026-03-01_facture-a.txt", date: "2026-05-20", basis: "header_label" },
      ],
    });

    expect(text).toContain("Parmi 3 fichiers lisibles de type « factures »");
    expect(text).toContain(
      "- medsupply-fac-020.txt — 2026-06-02 (date écrite dans le document, avec son intitulé)",
    );
    expect(text).not.toContain("dont le nom contient");
  });

  it("says earliest, and does not name a noun that names no kind of document", () => {
    const text = formatFolderAnswer(english, {
      ...(ranked as Extract<FolderAnswer, { kind: "ranked_file" }>),
      latest: false,
      noun: null,
      mixedBasis: false,
      unranked: 0,
    });

    expect(text).toContain("Among 3 readable files, the earliest is the first of this list:");
    expect(text).not.toContain("whose name contains");
    expect(text).not.toContain("not always the date of the document");
  });

  it("says when no readable file answers", () => {
    const none = {
      ...(ranked as Extract<FolderAnswer, { kind: "ranked_file" }>),
      candidates: 0,
      ranking: [],
    };

    expect(formatFolderAnswer(english, none)).toBe(
      "No readable file whose name contains “courrier” in the selected documents.",
    );
    expect(formatFolderAnswer(english, { ...none, noun: null })).toBe(
      "No readable file in the selected documents.",
    );
  });
});
