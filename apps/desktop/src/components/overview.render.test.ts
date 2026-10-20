import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { mergeInventories } from "../lib/explorerModel";
import type { FileRecord, IndexSummary, ProcessingStatus } from "../lib/ipc";
import {
  overviewModel,
  type FamilyReportInput,
  type OverviewInput,
} from "../lib/overviewModel";
import { OverviewPage } from "./OverviewPage";

/* The page drawn to static markup with the real catalogues and the real model: what the user would
   read in each state. Clicks are not simulated: what a link asks for is a pure function of the model
   (`overviewModel.test.ts`), and the click itself belongs to the owner's human test. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

const NOOP = () => undefined;
const ACTIONS = {
  onShowUnreadable: NOOP,
  onOpenKnowledge: NOOP,
  onOpenFile: NOOP,
  onRevealFile: NOOP,
};

/** French separates thousands with a narrow no-break space: compared as an ordinary one. */
function plain(text: string): string {
  return text.replace(/[  ]/g, " ").replace(/&#x27;/g, "'");
}

function record(relativePath: string, status: ProcessingStatus = "indexed"): FileRecord {
  const name = relativePath.split("/").pop() ?? relativePath;
  const dot = name.lastIndexOf(".");
  return {
    id: `id-${relativePath}`,
    relativePath,
    name,
    stem: dot < 0 ? name : name.slice(0, dot),
    extension: dot < 0 ? "" : name.slice(dot + 1).toLowerCase(),
    sizeBytes: 10,
    modifiedAt: 1000,
    processingStatus: status,
  } as FileRecord;
}

function summary(overrides: Partial<IndexSummary> = {}): IndexSummary {
  return {
    scannedFiles: 0,
    indexedFiles: 0,
    unchangedFiles: 0,
    emptyFiles: [],
    ocrFiles: [],
    lowConfidenceFiles: [],
    removedFiles: [],
    renamedFiles: [],
    renameFailedFiles: [],
    failedFiles: [],
    unavailableCapabilities: [],
    unreadFormats: [],
    chunkCount: 0,
    timings: null,
    ...overrides,
  };
}

const NONE: FamilyReportInput = {
  folder: null,
  folderState: "none",
  summary: null,
  durationMs: null,
  finishedAt: null,
  running: false,
  error: null,
  meaningPercent: null,
  needsAnalyse: false,
};
const FILLED: FamilyReportInput = { ...NONE, folder: "/folder", folderState: "filled" };

function drawn(
  locale: string,
  documents: FileRecord[],
  data: FileRecord[],
  reports: { documents?: Partial<FamilyReportInput>; tables?: Partial<FamilyReportInput> } = {},
  folders: "both" | "none" = "both",
): string {
  const input: OverviewInput =
    folders === "none"
      ? { entries: [], documents: NONE, tables: NONE }
      : {
          entries: mergeInventories(documents, data).entries,
          documents: { ...FILLED, ...reports.documents },
          tables: { ...FILLED, ...reports.tables },
        };
  const element: ReactElement = createElement(OverviewPage, {
    page: overviewModel(input),
    actions: ACTIONS,
  });
  return plain(renderToStaticMarkup(createElement(I18nProvider, { locale, children: element })));
}

describe("the page with no folder", () => {
  it("gives the first-start guidance and points at the left", () => {
    const html = drawn("fr-FR", [], [], {}, "none");

    expect(html).toContain("Pour commencer");
    expect(html).toContain("Choisissez deux dossiers.");
    expect(html).toContain("Déposez-y vos fichiers.");
    expect(html).toContain("Appuyez sur Analyser.");
    expect(html).toContain("dans la liste de gauche");
    expect(html).not.toContain("overview__card");
  });
});

describe("the page before the first analysis", () => {
  it("shows the three steps, with the files found, and one card per family", () => {
    const html = drawn("fr-FR", [record("a.pdf", "discovered"), record("b.docx", "discovered")], [
      record("t.csv", "discovered"),
    ]);

    expect(html).toContain("Avant la première analyse");
    expect(html).toContain("2 sur 2 dossiers choisis.");
    expect(html).toContain("3 fichiers trouvés.");
    expect(html).toContain("rien n&#x27;est envoyé sur Internet".replace("&#x27;", "'"));
    expect(html).toContain("Formats lus : PDF, DOCX, ODT…");
    expect(html).toContain("Formats lus : CSV, XLSX, ODS…");
    expect(html).toContain("2 fichiers attendent d'être analysés.");
    expect(html).toContain("1 fichier attend d'être analysé.");
    // The first two steps are done, the third is the one to do.
    expect(html.match(/overview__step--done/g)).toHaveLength(2);
    expect(html.match(/aria-current="step"/g)).toHaveLength(1);
    expect(html).not.toContain("Où en sont vos fichiers");
  });

  it("says a family has no folder, or an empty one", () => {
    const html = plain(
      renderToStaticMarkup(
        createElement(I18nProvider, {
          locale: "fr-FR",
          children: createElement(OverviewPage, {
            page: overviewModel({
              entries: [],
              documents: { ...FILLED, folderState: "empty" },
              tables: NONE,
            }),
            actions: ACTIONS,
          }),
        }),
      ),
    );

    expect(html).toContain("Dossier vide : déposez-y vos fichiers.");
    expect(html).toContain("Aucun dossier choisi : voir la liste de gauche.");
    expect(html).toContain("1 sur 2 dossiers choisis.");
    expect(html).toContain("Aucun fichier trouvé pour l'instant.");
  });

  it("gives the whole path of each folder in its card", () => {
    const html = drawn("fr-FR", [record("a.pdf", "discovered")], [], {
      documents: { folder: "C:\\Users\\x\\Docs" },
      tables: { folder: "C:\\Users\\x\\Data" },
    });

    expect(html).toContain("C:\\Users\\x\\Docs");
    expect(html).toContain("C:\\Users\\x\\Data");
  });

  it("says the first analysis is running", () => {
    const html = drawn("fr-FR", [record("a.pdf", "discovered")], [], {
      documents: { running: true },
    });

    expect(html).toContain("Analyse en cours…");
  });
});

describe("the page after an analysis", () => {
  const failing = summary({
    indexedFiles: 2,
    failedFiles: [{ path: "scan.pdf", code: "x_unknown_code", data: {} }],
    emptyFiles: ["blank.pdf"],
    removedFiles: ["gone-1.pdf", "gone-2.pdf"],
    renamedFiles: Array.from({ length: 8 }, (_, index) => ({
      from: `Mon fichier ${index}.pdf`,
      to: `Mon-fichier-${index}.pdf`,
    })),
    ocrFiles: ["photo.jpg"],
    unreadFormats: [{ extension: "ppt", count: 1 }],
    unavailableCapabilities: ["ocrEngine"],
  });
  const records = [
    record("a.pdf"),
    record("b.pdf"),
    record("blank.pdf", "failed"),
    record("scan.pdf", "discovered"),
    record("sub/old.pdf", "failed"),
  ];

  function report(locale = "fr-FR") {
    return drawn(locale, records, [record("t.csv")], {
      documents: { summary: failing, durationMs: 409_000, finishedAt: 1_700_000_000_000, meaningPercent: 63 },
      tables: { summary: summary({ indexedFiles: 1 }), durationMs: 2000, finishedAt: 1_700_000_100_000 },
    });
  }

  it("gives the whole path of each folder beside its figures", () => {
    const html = drawn("fr-FR", [record("a.pdf")], [record("t.csv")], {
      documents: { folder: "C:\\Users\\x\\Docs" },
      tables: { folder: "/home/x/Data" },
    });

    expect(html).toContain('class="overview__path" title="C:\\Users\\x\\Docs"');
    expect(html).toContain("/home/x/Data");
  });

  it("gives the figures of each family, the last pass and the meaning share", () => {
    const html = report();

    expect(html).toContain("Où en sont vos fichiers");
    expect(html).toMatch(/2 analysés/);
    expect(html).toMatch(/2 illisibles/);
    expect(html).toMatch(/1 à analyser/);
    expect(html).toMatch(/Dernière analyse : [^<]+ · 6 min 49 s/);
    expect(html).toContain("Recherche par sens : 63 %.");
  });

  it("lists the files to check with the reason of each, and a link to the left list", () => {
    const html = report();

    expect(html).toContain("À vérifier");
    expect(html).toContain("3 fichiers n'ont pas pu être lus.");
    expect(html).toContain("blank.pdf");
    expect(html).toContain("Aucun texte trouvé dans ce fichier.");
    expect(html).toContain("scan.pdf");
    expect(html).toContain("sub");
    expect(html).toContain("Afficher les 2 dans la liste de gauche");
    // No pass of this session says why `old.pdf` is unreadable: the page says so, it does not guess.
    // No pass of this session says why `old.pdf` is unreadable: the index does (no text found).
    expect(html).toContain("Aucun texte trouvé dans ce fichier.");
    expect(html).not.toContain("Raison connue");
    // A file the pass failed on shows its reason, never the raw code.
    expect(html).not.toContain("x_unknown_code");
    // The gates of the rows: Voir for a file of its own folder, no Ouvrir for an unreadable one.
    expect(html).toContain(">Voir</button>");
    expect(html).not.toContain(">Ouvrir</button>");
  });

  it("lists every cleaned name, the files read by text recognition and the unread formats", () => {
    const html = report();

    expect(html).toContain("Retirés du dossier");
    expect(html).toContain("2 documents retirés du dossier, oubliés de l'analyse.");
    expect(html).toContain("Noms nettoyés");
    for (let index = 0; index < 8; index += 1) {
      expect(html).toContain(`Mon fichier ${index}.pdf → Mon-fichier-${index}.pdf`);
    }
    expect(html).toContain("Lus par reconnaissance de texte");
    expect(html).toContain("photo.jpg");
    expect(html).toContain("Formats non lus");
    expect(html).toContain("1 .ppt");
    expect(html).toContain("ne sont pas listés à gauche");
  });

  it("names the capability that did not start and where the time went", () => {
    const html = report();

    expect(html).toContain("Fonctions qui n'ont pas démarré");
    expect(html).toContain("La reconnaissance de texte n'a pas démarré");
    expect(html).toContain("Où est passé le temps");
    expect(html).toContain("Total : 6 min 49 s");
  });

  it("draws a heading once for the two families, and a tag per family", () => {
    const html = drawn("fr-FR", [record("a.pdf")], [record("t.csv")], {
      documents: {
        summary: summary({ renamedFiles: [{ from: "a b.pdf", to: "a-b.pdf" }] }),
      },
      tables: { summary: summary({ renamedFiles: [{ from: "c d.csv", to: "c-d.csv" }] }) },
    });

    expect(html.match(/Noms nettoyés/g)).toHaveLength(1);
    expect(html).toContain("a b.pdf → a-b.pdf");
    expect(html).toContain("c d.csv → c-d.csv");
    expect(html.match(/overview__family overview__family--/g)?.length).toBeGreaterThanOrEqual(4);
  });

  it("is clean when nothing is to check: no list, no link", () => {
    const html = drawn("fr-FR", [record("a.pdf")], [], {
      documents: { summary: summary({ indexedFiles: 1 }), finishedAt: 1_700_000_000_000, durationMs: 5000 },
    });

    expect(html).not.toContain("À vérifier");
    expect(html).not.toContain("Afficher les");
    expect(html).not.toContain("Noms nettoyés");
  });

  it("says a stopped pass is stopped and that what was read is kept", () => {
    const html = drawn("fr-FR", [record("a.pdf")], [], {
      documents: {
        summary: summary({ cancelled: true, indexedFiles: 4 }),
        durationMs: 228_000,
        finishedAt: 1_700_000_000_000,
      },
    });

    expect(html).toMatch(/Analyse arrêtée : [^<]+ · 3 min 48 s/);
    expect(html).toContain("Ce qui est lu est conservé");
  });

  it("says nothing of a last pass for an index analysed before this session", () => {
    const html = drawn("fr-FR", [record("a.pdf")], []);

    expect(html).toContain("Analysé avant cette session.");
    expect(html).not.toContain("Dernière analyse");
    // The family with no analysed file does not claim an earlier analysis.
    expect(html).toContain("Pas encore analysé : appuyez sur Analyser.");
  });

  it("tells a pass that failed before it reported", () => {
    const html = drawn("fr-FR", [record("a.pdf")], [], {
      documents: { error: { code: "gateway_unreachable", data: {} } },
    });

    expect(html).toContain('role="alert"');
    expect(html).toContain("L'analyse n'a pas abouti : ");
    expect(html).not.toContain("gateway_unreachable");
  });

  it("says the same things in English", () => {
    const html = report("en-US");

    expect(html).toContain("Where your files stand");
    expect(html).toContain("To check");
    expect(html).toContain("Show the 2 in the list on the left");
    expect(html).toContain("Cleaned names");
    expect(html).toContain("Where the time went");
    expect(html).toContain("Total: 6 min 49 s");
  });

  it("keeps no scroll area of its own beyond the page's", () => {
    const html = report();

    expect(html).not.toContain("overflow");
    expect(html.startsWith('<div class="overview">')).toBe(true);
  });
});
