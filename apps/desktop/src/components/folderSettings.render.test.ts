import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { countsByFamily, mergeInventories, type ExplorerEntry } from "../lib/explorerModel";
import type { FileRecord, ProcessingStatus } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";
import { ExplorerFamilySection } from "./ExplorerFamilySection";
import { SETTINGS_LIST_ROWS } from "../config/layout";
import { FolderSettings } from "./FolderSettings";

/* The folder sections of the Settings panel, drawn to static markup with the real catalogues. They are
   the sidebar's family sections without the selection: what is absent matters as much as what is
   there. Clicks are not simulated; the folder commands and the passes are the sidebar's. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

const NOOP = () => undefined;

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

function page(locale: string, element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

function settings(overrides: Record<string, unknown> = {}, locale = "fr-FR") {
  return page(
    locale,
    createElement(FolderSettings, {
      workFolder: "C:\\Users\\her\\Docs",
      dataFolder: "C:\\Users\\her\\Data",
      suggestedWorkFolder: null,
      suggestedDataFolder: null,
      onWorkFolderChosen: NOOP,
      onDataFolderChosen: NOOP,
      indexing: IDLE,
      dataIndexing: IDLE,
      density: "compact",
      onCleared: NOOP,
      onOpenReport: NOOP,
      ...overrides,
    }),
  );
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

function entriesOf(documents: FileRecord[], data: FileRecord[]): ExplorerEntry[] {
  return mergeInventories(documents, data).entries;
}

/** One family's section as the Settings panel draws it: no selection, no fold, no report link. */
function section(entries: ExplorerEntry[], overrides: Record<string, unknown> = {}) {
  return page(
    "fr-FR",
    createElement(ExplorerFamilySection, {
      family: "documents",
      folder: "/some/folder",
      isEmpty: false,
      folderActions: { reveal: NOOP, change: NOOP, clear: NOOP },
      counts: countsByFamily(entries).documents,
      entries,
      allEntries: entries,
      open: true,
      rows: SETTINGS_LIST_ROWS,
      rowHeight: 24,
      indexing: IDLE,
      now: 0,
      onStop: NOOP,
      onRevealFile: NOOP,
      needsAnalyse: false,
      filtering: false,
      ...overrides,
    }),
  );
}

describe("the folders of the Settings panel, drawn", () => {
  it("draws the two families of the sidebar, each with its Analyser button and its folder menu", () => {
    const html = settings();

    expect(html.match(/<section class="explorer-family /g)).toHaveLength(2);
    expect(html).toContain("explorer-family--documents");
    expect(html).toContain("explorer-family--tables");
    expect(html.match(/>Analyser</g)).toHaveLength(2);
    expect(html).toContain('aria-label="Actions sur le dossier : Documents"');
    expect(html).toContain('aria-label="Actions sur le dossier : Tableaux"');
    expect(html).toContain('aria-label="Dossier des Documents"');
    expect(html).toContain('aria-label="Dossier des Tableaux"');
  });

  it("draws no selection, no search, no filter and no fold", () => {
    const html = settings();

    expect(html).not.toContain('type="checkbox"');
    expect(html).not.toContain('type="search"');
    expect(html).not.toContain(">Filtres<");
    expect(html).not.toContain("explorer__segments");
    expect(html).not.toContain("explorer-bottom");
    expect(html).not.toContain("explorer-family__fold");
  });

  it("offers to choose or make a folder for a family that has none, as the sidebar does", () => {
    const html = settings({
      workFolder: null,
      dataFolder: null,
      suggestedWorkFolder: "C:\\Users\\her\\AssistantCabinetAI\\Docs",
      suggestedDataFolder: "C:\\Users\\her\\AssistantCabinetAI\\Data",
    });

    expect(html.match(/explorer-setup__actions/g)).toHaveLength(2);
    expect(html.match(/>Créer et utiliser</g)).toHaveLength(2);
    expect(html.match(/>Choisir un dossier</g)).toHaveLength(2);
    expect(html).not.toContain(">Analyser<");
  });

  it("asks for the second folder alone when only one is chosen", () => {
    const html = settings({ dataFolder: null });

    expect(html.match(/<section class="explorer-family /g)).toHaveLength(2);
    expect(html).toContain("Tableaux : CSV, XLSX, ODS…");
    expect(html.match(/>Choisir un dossier</g)).toHaveLength(1);
    expect(html.match(/>Analyser</g)).toHaveLength(1);
  });

  it("shows the gauge and the Stop of a running pass, and no folder menu, as in the sidebar", () => {
    const html = settings({ indexing: { ...IDLE, running: true } });

    expect(html).toContain("Arrêter");
    expect(html.match(/explorer-family__menu/g)).toHaveLength(1);
    expect(html.match(/>Analyser</g)).toHaveLength(1);
  });

  it("reads in English with the English catalogue", () => {
    const html = settings({}, "en-US");

    expect(html).toContain('aria-label="Documents folder"');
    expect(html).toContain("Folder actions: Documents");
    expect(html).toContain(">Analyze<");
  });
});

describe("a family section without the selection, drawn", () => {
  const entries = entriesOf([record("a.pdf"), record("b.pdf", "failed")], []);
  const nested = entriesOf([record("patients/dupont/note.pdf")], []);

  it("shows the whole path of the folder and the whole path of each file, only when detailed", () => {
    const detailed = section(nested, { folder: "/home/her/Docs", detailed: true });
    const plain = section(nested, { folder: "/home/her/Docs" });

    expect(detailed).toContain("/home/her/Docs</span>");
    expect(detailed).toContain('<span class="explorer-row__dir">patients/dupont/</span>');
    expect(plain).not.toContain("/home/her/Docs</span>");
    expect(plain).toContain("…/Docs</span>");
    expect(plain).not.toContain("explorer-row__dir");
  });

  it("says where a file of the other family really sits, in full", () => {
    const foreign = entriesOf([record("grid.xlsx", "discovered")], []);
    const html = section(foreign, { family: "tables", detailed: true });

    expect(html).toContain("depuis Docs");
  });

  it("lists the files with no checkbox, and a click on a row ticks nothing", () => {
    const html = section(entries);

    expect(html).toContain("explorer-row__stem");
    expect(html).not.toContain('type="checkbox"');
    expect(html).not.toContain("explorer-row--tickable");
  });

  it("offers « Voir » on every row, and « Ouvrir » only when the panel can open a file", () => {
    const without = section(entries);
    const withOpen = section(entries, { onOpenFile: NOOP });

    expect(without.match(/>Voir</g)).toHaveLength(2);
    expect(without).not.toContain(">Ouvrir<");
    expect(withOpen.match(/>Voir</g)).toHaveLength(2);
    expect(withOpen.match(/>Ouvrir</g)).toHaveLength(1);
  });

  it("draws the fold button only when the list can be folded", () => {
    expect(section(entries)).not.toContain("explorer-family__fold");
    expect(section(entries, { onToggleOpen: NOOP })).toContain("explorer-family__fold");
  });

  it("keeps the buttons on the right: a row without a checkbox has its own column layout", () => {
    expect(section(entries)).toContain("explorer-row explorer-row--plain");
    expect(section(entries, { isTicked: () => false, onToggleTick: NOOP })).not.toContain(
      "explorer-row--plain",
    );
  });

  it("keeps the checkbox of the sidebar when a selection is given", () => {
    const html = section(entries, { isTicked: () => false, onToggleTick: NOOP });

    expect(html).toContain('type="checkbox"');
    expect(html).toContain("explorer-row--tickable");
  });
});
