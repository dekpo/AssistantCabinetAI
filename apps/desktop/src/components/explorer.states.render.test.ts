import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { noDocumentsScope } from "../lib/analysisScope";
import { countsByFamily } from "../lib/explorerModel";
import type { IndexingState } from "../state/useIndexing";
import { ExplorerFamilyMenu, ExplorerMenuList } from "./ExplorerFamilyMenu";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { ExplorerFamilySetup, ExplorerFirstStart } from "./ExplorerSetup";
import { NO_VIEW } from "../lib/explorerView";
import { FileExplorer } from "./FileExplorer";

/* The three situations of the box and its menu, drawn to static markup with the real catalogues. The
   choice of situation is `lib/explorerStates.ts`, tested where the logic is; here the words, the
   buttons and what is absent. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

const NOOP = () => undefined;

function page(locale: string, element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
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

function box(overrides: Record<string, unknown> = {}, locale = "fr-FR") {
  return page(
    locale,
    createElement(FileExplorer, {
      workFolder: null,
      dataFolder: null,
      suggestedWorkFolder: "C:\\Users\\her\\AssistantCabinetAI\\Docs",
      suggestedDataFolder: "C:\\Users\\her\\AssistantCabinetAI\\Data",
      onWorkFolderChosen: NOOP,
      onDataFolderChosen: NOOP,
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
      ...overrides,
    }),
  );
}

describe("the first start, drawn", () => {
  it("says the three steps in the owner's words and offers the two suggested folders", () => {
    const html = box();

    expect(html).toContain("Pour commencer");
    expect(html).toContain("<li>Choisissez deux dossiers.</li>");
    expect(html).toContain("<li>Déposez-y vos fichiers.</li>");
    expect(html).toContain("<li>Appuyez sur Analyser.</li>");
    expect(html).toContain("Créer les deux dossiers suggérés");
  });

  it("gives each family its type line, its short path and the two buttons side by side", () => {
    const html = box();

    expect(html.match(/<section class="explorer-family /g)).toHaveLength(2);
    expect(html).toContain("Documents : PDF, DOCX, ODT…");
    expect(html).toContain("Tableaux : CSV, XLSX, ODS…");
    expect(html).toContain("…\\Docs");
    expect(html).toContain("…\\Data");
    expect(html.match(/explorer-setup__actions/g)).toHaveLength(2);
    expect(html.match(/>Créer et utiliser</g)).toHaveLength(2);
    expect(html.match(/>Choisir un dossier</g)).toHaveLength(2);
  });

  it("puts the whole path and the reason on hover only, never on screen", () => {
    const html = box();

    expect(html).toContain("C:\\Users\\her\\AssistantCabinetAI\\Docs\n");
    expect(html).toContain("OneDrive ou iCloud");
    // The reason is in a title attribute, not in a paragraph of the box.
    expect(html).not.toMatch(/>[^<]*OneDrive[^<]*</);
  });

  it("writes no ellipsis character after the two buttons", () => {
    const html = box();

    expect(html).not.toMatch(/Choisir un dossier(\.\.\.|…)/);
    expect(html).not.toMatch(/Créer et utiliser(\.\.\.|…)/);
  });

  it("turns the search field and the Filtres button off, and draws no segmented control or bar", () => {
    const html = box();

    expect(html).toMatch(/<input[^>]*disabled=""[^>]*type="search"|<input[^>]*type="search"[^>]*disabled=""/);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Filtres<\/button>/);
    expect(html).not.toContain("explorer__segments");
    expect(html).not.toContain("explorer-bottom");
  });

  it("draws no Analyser button, no fold button and no menu: there is no folder yet", () => {
    const html = box();

    expect(html).not.toContain(">Analyser<");
    expect(html).not.toContain("explorer-family__fold");
    expect(html).not.toContain("explorer-family__menu");
  });

  it("offers only the choice when no suggestion is known", () => {
    const html = box({ suggestedWorkFolder: null, suggestedDataFolder: null });

    expect(html).not.toContain("Créer les deux dossiers suggérés");
    expect(html).not.toContain("Créer et utiliser");
    expect(html.match(/>Choisir un dossier</g)).toHaveLength(2);
  });

  it("reads in English with its own catalogue", () => {
    const html = box({}, "en-US");

    expect(html).toContain("Getting started");
    expect(html).toContain("Documents: PDF, DOCX, ODT…");
    expect(html).toContain("Create and use");
    expect(html).toContain("Choose a folder");
  });

  it("asks for the second folder alone when only one is chosen, with no first-start block", () => {
    const html = box({ workFolder: "C:\\Users\\her\\Docs" });

    expect(html).not.toContain("Pour commencer");
    expect(html).toContain("Tableaux : CSV, XLSX, ODS…");
    expect(html).not.toContain("Documents : PDF, DOCX, ODT…");
    expect(html.match(/>Choisir un dossier</g)).toHaveLength(1);
  });
});

describe("the setup block of one family, drawn", () => {
  const setup = (suggested: string | null) =>
    page(
      "fr-FR",
      createElement(ExplorerFamilySetup, {
        family: "documents",
        suggested,
        busy: false,
        onCreate: NOOP,
        onChoose: NOOP,
      }),
    );

  it("keeps the family's class, for its colours", () => {
    expect(setup("/a/Docs")).toContain("explorer-family--documents");
  });

  it("disables both buttons while a folder is being made or chosen", () => {
    const html = page(
      "fr-FR",
      createElement(ExplorerFamilySetup, {
        family: "tables",
        suggested: "/a/Data",
        busy: true,
        onCreate: NOOP,
        onChoose: NOOP,
      }),
    );

    expect(html.match(/disabled=""/g)).toHaveLength(2);
  });

  it("shows the block of steps with its button only when a suggestion exists", () => {
    const first = (canCreate: boolean) =>
      page(
        "fr-FR",
        createElement(ExplorerFirstStart, { busy: false, canCreate, onCreateBoth: NOOP }),
      );

    expect(first(true)).toContain("Créer les deux dossiers suggérés");
    expect(first(false)).not.toContain("Créer les deux dossiers suggérés");
    expect(first(false)).toContain("Appuyez sur Analyser.");
  });
});

function section(overrides: Record<string, unknown> = {}, family: "documents" | "tables" = "documents") {
  return page(
    "fr-FR",
    createElement(ExplorerFamilySection, {
      family,
      folder: "/some/folder",
      isEmpty: true,
      folderActions: { reveal: NOOP, change: NOOP, clear: NOOP },
      counts: countsByFamily([])[family],
      entries: [],
      allEntries: [],
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

describe("a folder created and still empty, drawn", () => {
  it("says it is empty, with the two things to do about it", () => {
    const html = section();

    expect(html).toContain("Vide. Déposez-y vos fichiers.");
    expect(html).toContain(">Voir le dossier<");
    expect(html).toContain(">Changer<");
    expect(html).not.toContain("Aucun fichier.");
    expect(html).not.toContain("explorer-list");
  });

  it("keeps the same type line as the first start, in place of the counters", () => {
    expect(section()).toContain("PDF, DOCX, ODT…");
    expect(section({}, "tables")).toContain("CSV, XLSX, ODS…");
    expect(section()).not.toContain("explorer-family__counters");
  });

  it("greys Analyser out with a tooltip that says why", () => {
    const html = section();

    expect(html).toMatch(/<button[^>]*title="Rien à analyser : ce dossier est vide[^"]*"[^>]*disabled=""/);
    expect(html).toContain(">Analyser<");
  });

  it("draws nothing under the header when it is folded", () => {
    const html = section({ open: false });

    expect(html).not.toContain("Vide. Déposez-y vos fichiers.");
  });

  it("is the ordinary list, with its counters, when the folder holds files", () => {
    const html = section({ isEmpty: false });

    expect(html).not.toContain("Vide. Déposez-y vos fichiers.");
    expect(html).toContain("explorer-family__counters");
  });
});

describe("the menu of a family, drawn", () => {
  it("is a closed « ⋯ » button, named after the family, until it is opened", () => {
    const html = section({ isEmpty: false });

    expect(html).toContain("explorer-family__menu");
    expect(html).toContain('aria-haspopup="menu"');
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain('aria-label="Actions sur le dossier : Documents"');
    expect(html).not.toContain('role="menu"');
  });

  it("is absent while that family is being analysed, and the fold button stays", () => {
    const running = section({ isEmpty: false, indexing: { ...IDLE, running: true } });

    expect(running).not.toContain("explorer-family__menu");
    expect(running).toContain("explorer-family__fold");
  });

  it("lists the three entries, in this order, when open", () => {
    const html = page(
      "fr-FR",
      createElement(ExplorerMenuList, {
        id: "m",
        label: "Actions sur le dossier : Documents",
        items: [
          { id: "reveal", label: "Voir le dossier", onSelect: NOOP },
          { id: "change", label: "Changer de dossier", onSelect: NOOP },
          { id: "clear", label: "Effacer l'analyse", onSelect: NOOP },
        ],
        placement: { top: 40, right: 12 },
        onSelect: NOOP,
      }),
    );

    expect(html).toContain('role="menu"');
    expect(html).toContain("top:40px;right:12px");
    const entries = [...html.matchAll(/role="menuitem"[^>]*>([^<]*)</g)].map((match) => match[1]);
    expect(entries).toEqual(["Voir le dossier", "Changer de dossier", "Effacer l&#x27;analyse"]);
  });

  it("is a button with no menu drawn under it at first", () => {
    const html = page(
      "fr-FR",
      createElement(ExplorerFamilyMenu, {
        label: "Actions sur le dossier : Tableaux",
        items: [{ id: "reveal", label: "Voir le dossier", onSelect: NOOP }],
      }),
    );

    expect(html).toContain("explorer-family__menu");
    expect(html).not.toContain("explorer-menu\"");
  });

  it("says the menu entries in English with the English catalogue", () => {
    const html = page(
      "en-US",
      createElement(ExplorerFamilySection, {
        family: "documents",
        folder: "/some/folder",
        isEmpty: true,
        folderActions: { reveal: NOOP, change: NOOP, clear: NOOP },
        counts: countsByFamily([]).documents,
        entries: [],
        allEntries: [],
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
      }),
    );

    expect(html).toContain("Empty. Drop your files in it.");
    expect(html).toContain("Show the folder");
    expect(html).toContain("Folder actions: Documents");
  });
});

describe("a family header says which folder it is", () => {
  it("shows the folder's short name beside the family's, the whole path in the tooltip", () => {
    const html = section({ folder: "C:\\Users\\x\\Docs" });

    expect(html).toContain('class="explorer-family__location"');
    expect(html).toContain("…\\Docs");
    expect(html).toContain('title="C:\\Users\\x\\Docs"');
    expect(html.indexOf("explorer-family__name")).toBeLessThan(html.indexOf("explorer-family__location"));
  });
});
