import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { noDocumentsScope } from "../lib/analysisScope";
import { folderChoices, formatChoices, stateCounts } from "../lib/explorerFilters";
import {
  countsByFamily,
  folderKey,
  mergeInventories,
  type ExplorerEntry,
  type ExplorerState,
} from "../lib/explorerModel";
import type { FileRecord, ProcessingStatus } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";
import { ExplorerBottomBar } from "./ExplorerBottomBar";
import { ExplorerActivePills } from "./ExplorerActivePills";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { ExplorerFilterPanel, FolderSuggestionList } from "./ExplorerFilterPanel";
import { ExplorerFileRow } from "./ExplorerFileRow";
import { NO_VIEW } from "../lib/explorerView";
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
  trace: null,
  durationMs: null,
  finishedAt: null,
  summary: null,
  error: null,
  finishedPasses: 0,
  run: async () => true,
  reset: () => undefined,
};

const NOOP = () => undefined;

/** The folder props the box asks of `App`, for a test that does not use them. */
const FOLDER_CHOICE = {
  suggestedWorkFolder: null,
  suggestedDataFolder: null,
  onWorkFolderChosen: NOOP,
  onDataFolderChosen: NOOP,
};

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

  it("is a row a click ticks only while the file can be ticked", () => {
    const [entry] = entriesOf([record("a.pdf")], []);

    expect(row(entry as ExplorerEntry)).toContain("explorer-row explorer-row--tickable");
    expect(row(entry as ExplorerEntry, { checkDisabled: true })).not.toContain("explorer-row--tickable");
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
    expect(sheetHtml).toContain("il est analysé là où il se trouve");
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
      isEmpty: false,
      folderActions: { reveal: NOOP, change: NOOP, clear: NOOP },
      counts: countsByFamily(all)[family],
      entries: all.filter((entry) => entry.family === family),
      allEntries: all,
      open: true,
      onToggleOpen: NOOP,
      rows: 6,
      rowHeight: 24,
      indexing: IDLE,
      now: 0,
      onStop: NOOP,
      locked: false,
      isTicked: () => false,
      onToggleTick: NOOP,
      onOpenFile: NOOP,
      onRevealFile: NOOP,
      needsAnalyse: false,
      onOpenReport: NOOP,
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

  it("counts in Analyser every file of the family that waits, wherever it sits", () => {
    // Two documents wait in the Documents folder; one more waits in the Data folder (read there).
    const html = section(
      [record("a.pdf", "discovered"), record("b.pdf", "pending"), record("c.pdf")],
      [record("lettre.docx", "discovered")],
    );

    expect(html).toContain("Analyser (3)");
    // No tooltip about a file left out: nothing is left out any more.
    expect(html).not.toContain("autre dossier n&#x27;est pas compté");
    expect(html).toContain('aria-label="3 à analyser"');
  });

  it("does not count a foreign file already analysed or unreadable as waiting", () => {
    const html = section(
      [record("a.pdf", "discovered")],
      [record("lettre.docx", "indexed"), record("note.txt", "failed")],
    );

    expect(html).toContain("Analyser (1)");
  });

  it("says plain Analyser, without a number, when nothing of the family waits", () => {
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
    expect(bar({ unsupported: 3 })).toContain("3 fichiers d&#x27;un format non lu ne sont pas listés");
  });
});

describe("the bottom bar with nothing used", () => {
  const bar = (overrides: Record<string, unknown> = {}) =>
    page(
      "fr-FR",
      createElement(ExplorerBottomBar, {
        documentsUsed: 0,
        tablesUsed: 0,
        toTick: 0,
        onTick: NOOP,
        canClear: false,
        onClear: NOOP,
        locked: false,
        notes: ["none"],
        unsupported: 1,
        ...overrides,
      }),
    );

  it("says « Aucun fichier pour l'IA » on the first line, so the bar stays at two lines", () => {
    const html = bar();
    const first = html.slice(html.indexOf("explorer-bottom__line"), html.indexOf("</div>"));

    expect(first).toContain("aucun");
    expect(first).toContain("Aucun fichier pour l&#x27;IA");
    // The only other line is the unread-format note: two lines in all.
    expect(html.match(/explorer-bottom__note/g)).toHaveLength(1);
    expect(html).not.toContain("les réponses ne s");
  });

  it("keeps the whole-family warnings on their own line", () => {
    const html = bar({ notes: ["wholeDocuments"], unsupported: 0, documentsUsed: 3 });

    expect(html).toContain("Tous les documents analysés sont utilisés.");
    expect(html).not.toContain("Aucun fichier pour l");
  });
});

describe("the whole box, drawn", () => {
  const explorer = () =>
    page(
      "fr-FR",
      createElement(FileExplorer, {
        workFolder: "/docs",
        dataFolder: "/data",
        ...FOLDER_CHOICE,
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
        view: NO_VIEW,
        onViewChange: NOOP,
        notesFolder: "Notes",
        searchClears: 0,
        onFacts: NOOP,
        onOpenReport: NOOP,
        savedViews: [],
        onSavedViewsChange: NOOP,
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

  it("shows a working Filtres button, closed, and the Fichiers and Vues switch both working", () => {
    const html = explorer();

    expect(html).toMatch(/<button[^>]*aria-expanded="false"[^>]*>Filtres<\/button>/);
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>Filtres/);
    expect(html).toMatch(/<button[^>]*aria-pressed="true"[^>]*>Fichiers<\/button>/);
    expect(html).toMatch(/<button[^>]*aria-pressed="false"[^>]*>Vues<\/button>/);
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>Vues<\/button>/);
    // The panel and the pills are drawn only when opened or when a filter is in force.
    expect(html).not.toContain("explorer-filters");
    expect(html).not.toContain("explorer-pills");
  });

  it("disables the Filtres button while no folder is chosen", () => {
    const html = page(
      "fr-FR",
      createElement(FileExplorer, {
        workFolder: null,
        dataFolder: null,
        ...FOLDER_CHOICE,
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
        view: NO_VIEW,
        onViewChange: NOOP,
        notesFolder: "Notes",
        searchClears: 0,
        onFacts: NOOP,
        onOpenReport: NOOP,
        savedViews: [],
        onSavedViewsChange: NOOP,
        onOpenDocument: NOOP,
        onOpenTable: NOOP,
        refreshToken: 0,
      }),
    );

    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Filtres<\/button>/);
  });

  it("carries the row height of the density as a custom property", () => {
    expect(explorer()).toContain("--explorer-row:24px");
  });
});

describe("a row with the reason of a hit, drawn", () => {
  const [entry] = entriesOf([record("a/rapport.pdf")], []);

  it("shows the short reason beside the name and every reason in its tooltip", () => {
    const html = row(entry as ExplorerEntry, {
      reason: { text: "dans le nom", title: "dans le nom, dans le contenu" },
    });

    expect(html).toContain(
      '<span class="explorer-row__reason" title="dans le nom, dans le contenu">dans le nom</span>',
    );
  });

  it("shows no reason without a text search", () => {
    expect(row(entry as ExplorerEntry)).not.toContain("explorer-row__reason");
  });
});

describe("the filter panel, drawn", () => {
  const entries = entriesOf(
    [record("clients/a.pdf"), record("clients/b.docx", "failed"), record("c.pdf", "discovered")],
    [record("budget/g.xlsx")],
  );
  const panel = (overrides: Record<string, unknown> = {}, locale = "fr-FR") =>
    page(
      locale,
      createElement(ExplorerFilterPanel, {
        id: "p",
        formats: formatChoices(entries, {}, []),
        chosenFormats: new Set<string>(),
        typedFormats: [],
        stateCounts: stateCounts(entries, {}),
        chosenStates: new Set<ExplorerState>(),
        folders: folderChoices(entries, {}),
        chosenFolders: new Set<string>(),
        disabled: false,
        anyActive: false,
        onToggle: NOOP,
        onRemoveTyped: NOOP,
        onClear: NOOP,
        onClose: NOOP,
        ...overrides,
      }),
    );

  it("has format and state pills with their counts, the folder field and the two buttons", () => {
    const html = panel();

    expect(html).toContain("Format");
    expect(html).toContain("État");
    expect(html).toContain("Dossier d&#x27;origine");
    expect(html).toMatch(/aria-pressed="false"[^>]*>\.pdf<small[^>]*>2<\/small>/);
    expect(html).toMatch(/>Analysé<small[^>]*>2<\/small>/);
    expect(html).toMatch(/>À analyser<small[^>]*>1<\/small>/);
    expect(html).toMatch(/>Illisible<small[^>]*>1<\/small>/);
    expect(html).toContain("Effacer les filtres");
    expect(html).toContain("Terminé");
    expect(html).toContain('type="search"');
  });

  it("has no scroll area, and draws the suggestions only while the field has the focus", () => {
    const html = panel();

    expect(html).not.toMatch(/overflow|scroll/i);
    expect(html).not.toContain("explorer-suggest");
  });

  it("lights a ticked pill and one that comes from the typed text, and disables clearing when nothing is active", () => {
    const html = panel({
      chosenFormats: new Set(["pdf"]),
      typedFormats: ["xlsx"],
      formats: formatChoices(entries, {}, ["xlsx"]),
    });

    expect(html).toMatch(/aria-pressed="true"[^>]*>\.pdf</);
    expect(html).toMatch(/aria-pressed="true"[^>]*title="[^"]*"[^>]*>\.xlsx</);
    expect(html).toMatch(/disabled=""[^>]*>Effacer les filtres/);
  });

  it("disables every control while there is nothing to filter", () => {
    const html = panel({ disabled: true });

    expect(html.match(/<button[^>]*class="explorer-chip[^>]*disabled=""/g)?.length).toBeGreaterThan(3);
  });

  it("says no sub-folder when the folders hold none", () => {
    const flat = entriesOf([record("a.pdf")], []);

    expect(panel({ folders: folderChoices(flat, {}) })).toContain("Aucun sous-dossier.");
  });

  it("speaks English with the English catalogue", () => {
    const html = panel({}, "en-US");

    expect(html).toContain("Origin folder");
    expect(html).toContain("Clear filters");
  });
});

describe("the suggestions of the folder field, drawn", () => {
  const many = entriesOf(
    Array.from({ length: 8 }, (_, index) => record(`dir${index}/f.pdf`)),
    [],
  );
  const list = (typed: string, locale = "fr-FR") =>
    page(
      locale,
      createElement(FolderSuggestionList, {
        choices: folderChoices(many, {}),
        typed,
        chosen: new Set<string>(),
        onPick: NOOP,
      }),
    );

  it("lists at most five folders with their counts, then says how many more", () => {
    const html = list("");

    expect(html.match(/explorer-suggest__item/g)).toHaveLength(5);
    expect(html).toContain("3 autres : tapez pour affiner");
  });

  it("says nothing more when everything fits, and says when nothing matches", () => {
    expect(list("dir7")).not.toContain("autre");
    expect(list("nothing-like-it")).toContain("Aucun dossier ne correspond.");
  });
});

describe("the active pills, drawn", () => {
  const folders = folderChoices(entriesOf([record("2025/a.pdf")], [record("2025/t.csv")]), {});
  const pills = (items: Parameters<typeof ExplorerActivePills>[0]["pills"]) =>
    page(
      "fr-FR",
      createElement(ExplorerActivePills, { pills: items, folders, onRemove: NOOP, onClear: NOOP }),
    );

  it("draws nothing without a filter", () => {
    expect(pills([])).toBe("");
  });

  it("draws a removable pill per filter and no clear-all link for a single one", () => {
    const html = pills([{ kind: "format", value: "pdf", typed: false }]);

    expect(html).toContain(">.pdf<");
    expect(html).toContain('aria-label="Retirer le filtre .pdf"');
    expect(html).not.toContain("Tout effacer");
  });

  it("offers Tout effacer from two pills, and says the root of a folder both roots hold", () => {
    const html = pills([
      { kind: "state", value: "unreadable", typed: false },
      { kind: "folder", value: folderKey("data", "2025"), typed: false },
    ]);

    expect(html).toContain(">Illisible<");
    expect(html).toContain("2025 (Data)");
    expect(html).toContain("Tout effacer");
  });
});
