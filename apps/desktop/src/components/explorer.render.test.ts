import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { noDocumentsScope } from "../lib/analysisScope";
import { countsByFamily, mergeInventories, type ExplorerEntry } from "../lib/explorerModel";
import type { FileRecord, ProcessingStatus } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";
import { ExplorerBottomBar } from "./ExplorerBottomBar";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { ExplorerFileRow } from "./ExplorerFileRow";
import { FileExplorer } from "./FileExplorer";

/* Each component is drawn to static markup with the real catalogues, to read what the user would
   see. Clicks are not simulated: what a click asks for lives in `lib/explorerSelection.ts` and
   `lib/explorerModel.ts`, which are tested where the logic is. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(locale: string, element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

function record(relativePath: string, status: ProcessingStatus = "indexed"): FileRecord {
  const name = relativePath.split("/").pop() ?? relativePath;
  const dot = name.lastIndexOf(".");
  return {
    id: `id-${relativePath}`,
    relativePath,
    name,
    stem: name.slice(0, dot),
    extension: name.slice(dot + 1),
    sizeBytes: 1,
    modifiedAt: 1,
    processingStatus: status,
  } as FileRecord;
}

const IDLE: IndexingState = {
  running: false,
  progress: null,
  summary: null,
  error: null,
  finishedPasses: 0,
  run: async () => true,
  reset: () => undefined,
};

const NOOP = () => undefined;

function row(entry: ExplorerEntry, overrides: Record<string, unknown> = {}, locale = "fr-FR") {
  return page(
    locale,
    createElement("ul", null, [
      createElement(ExplorerFileRow, {
        key: entry.key,
        entry,
        checked: false,
        checkDisabled: false,
        onToggle: NOOP,
        hint: "",
        ...overrides,
      }),
    ]),
  );
}

function entriesOf(documents: FileRecord[], data: FileRecord[]): ExplorerEntry[] {
  return mergeInventories(documents, data).entries;
}

const EMOJI_OR_TEXT_GLYPH = /\p{Extended_Pictographic}|[●○▲▾■]/u;

describe("a file row, drawn", () => {
  it("shows the stem, then the extension as a badge, even for a very long name", () => {
    const long = `${"a-very-long-file-name-".repeat(8)}end.docx`;
    const [entry] = entriesOf([record(long)], []);
    const html = row(entry as ExplorerEntry);

    expect(html).toContain('<span class="explorer-row__stem">');
    // The badge comes after the stem and is its own element: only the stem carries the ellipsis.
    expect(html.indexOf("explorer-row__ext")).toBeGreaterThan(html.indexOf("explorer-row__stem"));
    expect(html).toContain('<span class="explorer-row__ext">.docx</span>');
  });

  it("puts the full relative path in the tooltip", () => {
    const [entry] = entriesOf([record("patients/dupont/note.pdf")], []);

    expect(row(entry as ExplorerEntry)).toContain('title="patients/dupont/note.pdf"');
  });

  it("tags a file that sits in the other family's folder, with a tooltip saying it cannot be analysed yet", () => {
    const [sheet] = entriesOf([record("grid.xlsx", "discovered")], []);
    const [letter] = entriesOf([], [record("lettre.docx", "failed")]);

    const sheetHtml = row(sheet as ExplorerEntry, { checkDisabled: true });
    expect(sheetHtml).toContain("depuis Docs");
    expect(sheetHtml).toContain("ne peut pas encore être analysé ici");
    expect(sheetHtml).toContain("explorer-row--foreign");
    expect(row(letter as ExplorerEntry, { checkDisabled: true })).toContain("depuis Data");
  });

  it("does not tag a file of its own folder", () => {
    const [entry] = entriesOf([record("a.pdf")], []);
    const html = row(entry as ExplorerEntry);

    expect(html).not.toContain("explorer-row__tag");
    expect(html).not.toContain("depuis");
  });

  it("says each state in words for a screen reader, whatever the shape", () => {
    const [analysed] = entriesOf([record("a.pdf", "indexed")], []);
    const [waiting] = entriesOf([record("b.pdf", "pending")], []);
    const [unreadable] = entriesOf([record("c.pdf", "failed")], []);

    expect(row(analysed as ExplorerEntry)).toContain('aria-label="Analysé"');
    expect(row(waiting as ExplorerEntry)).toContain('aria-label="À analyser"');
    expect(row(unreadable as ExplorerEntry)).toContain('aria-label="Illisible"');
    // Three different shapes: a disc, a ring, a triangle.
    expect(row(analysed as ExplorerEntry)).toContain("<circle");
    expect(row(unreadable as ExplorerEntry)).toContain("<path");
  });

  it("shows the grey sub-folder only when it is given", () => {
    const [entry] = entriesOf([record("patients/dupont/note.pdf")], []);

    expect(row(entry as ExplorerEntry, { hint: "dupont" })).toContain(
      '<span class="explorer-row__hint" title="patients/dupont">dupont</span>',
    );
    expect(row(entry as ExplorerEntry)).not.toContain("explorer-row__hint");
  });

  it("offers Ouvrir and Voir only where they are given, and draws no emoji", () => {
    const [entry] = entriesOf([record("a.pdf")], []);
    const bare = row(entry as ExplorerEntry);
    const full = row(entry as ExplorerEntry, { onOpen: NOOP, onReveal: NOOP });

    expect(bare).not.toContain("Ouvrir");
    expect(full).toContain("Ouvrir");
    expect(full).toContain("Voir");
    expect(full).not.toMatch(EMOJI_OR_TEXT_GLYPH);
  });

  it("is written in both languages", () => {
    const [entry] = entriesOf([], [record("lettre.docx")]);

    expect(row(entry as ExplorerEntry, {}, "en-US")).toContain("from Data");
  });
});

function section(
  documents: FileRecord[],
  data: FileRecord[],
  overrides: Record<string, unknown> = {},
  family: "documents" | "tables" = "documents",
) {
  const all = entriesOf(documents, data);
  return page(
    "fr-FR",
    createElement(ExplorerFamilySection, {
      family,
      folder: "/some/folder",
      counts: countsByFamily(all)[family],
      entries: all.filter((entry) => entry.family === family),
      allEntries: all,
      open: true,
      onToggleOpen: NOOP,
      rows: 6,
      rowHeight: 24,
      indexing: IDLE,
      locked: false,
      isTicked: () => false,
      onToggleTick: NOOP,
      onOpenFile: NOOP,
      onRevealFile: NOOP,
      needsAnalyse: false,
      meaningPercent: null,
      passLines: [],
      showKnowledgeLink: false,
      filtering: false,
      ...overrides,
    }),
  );
}

describe("a family section, drawn", () => {
  it("carries its family's class and a different folder icon for each family", () => {
    const docs = section([record("a.pdf")], []);
    const tables = section([], [record("t.csv")], {}, "tables");

    expect(docs).toContain("explorer-family--documents");
    expect(docs).toContain("family-glyph--documents");
    expect(tables).toContain("explorer-family--tables");
    expect(tables).toContain("family-glyph--tables");
  });

  it("counts in Analyser only the files of its own folder that wait, never a foreign one", () => {
    // Two documents wait in the Documents folder; one more waits in the Data folder (foreign).
    const html = section(
      [record("a.pdf", "discovered"), record("b.pdf", "pending"), record("c.pdf")],
      [record("lettre.docx", "failed")],
    );

    expect(html).toContain("Analyser (2)");
    // The tooltip of the button says the foreign file is not counted.
    expect(html).toContain("1 fichier rangé dans l&#x27;autre dossier n&#x27;est pas compté");
    // ... while the header's "to analyse" counter does include it: three files wait.
    expect(html).toContain('aria-label="3 à analyser"');
  });

  it("says plain Analyser, without a number, when nothing of its own folder waits", () => {
    const html = section([record("a.pdf")], [record("lettre.docx")]);

    expect(html).toContain(">Analyser<");
    expect(html).not.toContain("Analyser (");
  });

  it("draws a thousand files as a few dozen rows inside a bounded scroll area", () => {
    const many = Array.from({ length: 1000 }, (_, index) => record(`doc-${index}.pdf`));
    const html = section(many, []);
    const rows = html.match(/<li /g) ?? [];

    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(30);
    // Six rows of 24 px, and the rest of the thousand as padding.
    expect(html).toContain("height:144px");
    expect(html).toContain("padding-bottom:");
  });

  it("is only its header when folded", () => {
    const html = section([record("a.pdf")], [], { open: false });

    expect(html).toContain("explorer-family__head");
    expect(html).not.toContain("<li ");
    expect(html).not.toContain("explorer-list");
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain("Déplier Documents");
  });

  it("says why a list is empty", () => {
    expect(section([], [])).toContain("Aucun fichier.");
    expect(section([], [], { filtering: true })).toContain("Aucun fichier ne correspond.");
    expect(section([], [], { folder: null })).toContain("Aucun dossier choisi");
  });

  it("shows no Analyser button while no folder is chosen", () => {
    expect(section([], [], { folder: null })).not.toContain("Analyser");
  });

  it("keeps the existing progress line and Stop while a pass runs", () => {
    const running: IndexingState = {
      ...IDLE,
      running: true,
      progress: {
        totalFiles: 10,
        processedFiles: 4,
        batchIndex: 0,
        batchTotal: 0,
      } as IndexingState["progress"],
    };
    const html = section([record("a.pdf")], [], { indexing: running, onStop: NOOP });

    expect(html).toContain('role="progressbar"');
    expect(html).toContain("Arrêter");
  });

  it("draws no emoji and no text glyph for a state", () => {
    const html = section(
      [record("a.pdf"), record("b.pdf", "failed"), record("c.pdf", "pending")],
      [record("lettre.docx")],
    );

    expect(html).not.toMatch(EMOJI_OR_TEXT_GLYPH);
  });
});

describe("the bottom bar, drawn", () => {
  function bar(overrides: Record<string, unknown> = {}) {
    return page(
      "fr-FR",
      createElement(ExplorerBottomBar, {
        documentsUsed: 2,
        tablesUsed: 0,
        toTick: 0,
        onTick: NOOP,
        canClear: true,
        onClear: NOOP,
        locked: false,
        notes: [],
        unsupported: 0,
        ...overrides,
      }),
    );
  }

  it("says how many documents and tables are used", () => {
    expect(bar()).toContain("Utilisés :");
    expect(bar()).toContain("2 doc. · 0 tab.");
    expect(bar({ documentsUsed: 0, canClear: false })).toContain("aucun");
  });

  it("offers the two small links only when they have something to do", () => {
    const quiet = bar({ canClear: false });
    expect(quiet).not.toContain("Cocher");
    expect(quiet).not.toContain("Décocher");

    const busy = bar({ toTick: 7 });
    expect(busy).toContain("Cocher les 7");
    expect(busy).toContain("Décocher");
    expect(bar({ toTick: 1 })).toContain("Cocher le fichier");
  });

  it("draws a second line only when there is something to say", () => {
    expect(bar()).not.toContain("explorer-bottom__note");
    expect(bar({ notes: ["none"] })).toContain("les réponses ne s&#x27;appuient pas sur vos fichiers");
    expect(bar({ unsupported: 3 })).toContain("3 fichiers d&#x27;un format non lu ne sont pas listés");
  });
});

describe("the whole box, drawn", () => {
  const explorer = () =>
    page(
      "fr-FR",
      createElement(FileExplorer, {
        workFolder: "/docs",
        dataFolder: "/data",
        indexing: IDLE,
        dataIndexing: IDLE,
        scope: noDocumentsScope(1),
        dataScope: noDocumentsScope(1),
        onScopeChange: NOOP,
        onDataScopeChange: NOOP,
        scopeLocked: false,
        density: "compact",
        folded: { documents: false, tables: false },
        onFoldedChange: NOOP,
        onOpenKnowledge: NOOP,
        onOpenDocument: NOOP,
        onOpenTable: NOOP,
        refreshToken: 0,
      }),
    );

  it("is one box with two family sections, the search field, the segmented control and the bottom bar", () => {
    const html = explorer();

    expect(html.match(/<section class="explorer-family /g)).toHaveLength(2);
    expect(html).toContain("Mes fichiers");
    expect(html).toContain('type="search"');
    expect(html).toContain("Tout");
    expect(html).toContain("Documents");
    expect(html).toContain("Tableaux");
    expect(html).toContain("explorer-bottom");
    // Nothing is drawn below the box: the markup ends with the bar's closing tags.
    expect(html.trimEnd().endsWith("</div></section>")).toBe(true);
  });

  it("shows the Filtres button and the Vues switch, disabled until their step", () => {
    const html = explorer();

    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Filtres<\/button>/);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Vues<\/button>/);
  });

  it("carries the row height of the density as a custom property", () => {
    expect(explorer()).toContain("--explorer-row:24px");
  });
});
