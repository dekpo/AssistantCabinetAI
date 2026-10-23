import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { DEFAULT_SPEECH_SETTINGS, type AppSettings } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";
import { SettingsDialog } from "./SettingsDialog";

/* The Settings panel drawn whole, to static markup with the real catalogues: which sections it has,
   in which order, and which blocks take the whole row. The folders and the voice have their own
   tests; here it is the arrangement the owner asked for (21 October 2026). */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));
vi.mock("../state/useSpeechStatus", () => ({ useSpeechStatus: () => ({ status: null, failed: false }) }));
vi.mock("../state/useModelJob", () => ({
  useModelJob: () => ({
    job: { kind: "idle" },
    target: null,
    install: () => {},
    verify: () => {},
    cancel: () => {},
  }),
}));

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

const SETTINGS = {
  locale: "fr-FR",
  theme: "system",
  serverUrl: "http://localhost:8080",
  modelAlias: "assistant",
  answerIdleTimeoutSeconds: 300,
  generatedFolderName: "Courriers",
  workFolder: "/home/her/Docs",
  dataFolder: "/home/her/Data",
  knowledgeMode: "suggest",
  tabularRowsMode: "sample",
  spreadsheetApplication: "excel",
  knowledgePacks: [],
  knowledgeHideNamesInSuggestions: true,
  copySources: false,
  knowledgeMinScopeFiles: 3,
  knowledgeMaxSelectivity: 0.5,
  knowledgeMinReduction: 0.3,
  knowledgeMaxCandidates: 20,
  knowledgeWidenBelowScore: 0.65,
  writeTimingLog: false,
  showDiagnostics: false,
  explorer: { density: "compact", documentsFolded: false, tablesFolded: false },
  speech: { ...DEFAULT_SPEECH_SETTINGS },
} as unknown as AppSettings;

function dialog(locale = "fr-FR"): string {
  return renderToStaticMarkup(
    createElement(I18nProvider, {
      locale,
      children: createElement(SettingsDialog, {
        settings: SETTINGS,
        settingsPath: "/settings.json",
        suggestedWorkFolder: null,
        suggestedDataFolder: null,
        aliases: [],
        saveError: null,
        indexing: IDLE,
        dataIndexing: IDLE,
        onUpdate: NOOP,
        onWorkFolderChosen: NOOP,
        onSaveExplorer: NOOP,
        onSaveSpeech: NOOP,
        onReset: async () => undefined,
        onRestoreLayout: NOOP,
        onClose: NOOP,
        onOpenKnowledge: NOOP,
        onOpenReport: NOOP,
        onFolderCleared: NOOP,
      }),
    }),
  );
}

/** The position of a label in the markup, as a label (not as a word inside a description). */
function at(html: string, label: string): number {
  return html.indexOf(`>${label}<`);
}

describe("the settings panel, drawn", () => {
  it("is as wide as the knowledge dialog and laid out in a grid of two columns", () => {
    const html = dialog();

    expect(html).toContain('class="dialog dialog--wide"');
    expect(html).toContain('class="dialog__body settings-grid"');
  });

  it("has its four sections in order, each with a title", () => {
    const html = dialog();
    const titles = ["Section principale", "Base de connaissances", "Gestion de la voix", "Disposition et réinitialisation"];
    const positions = titles.map((title) => at(html, title));

    expect(positions.every((position) => position >= 0)).toBe(true);
    expect([...positions].sort((a, b) => a - b)).toStrictEqual(positions);
    expect(html).toContain('<h3 class="dialog__subtitle settings-wide">Section principale</h3>');
  });

  it("offers the data sent to the AI from spreadsheets beside the smart targeting, sample selected", () => {
    const html = dialog();
    const targeting = at(html, "Ciblage intelligent");
    const sent = at(html, "Données envoyées à l&#x27;IA depuis vos tableurs");

    expect(targeting).toBeGreaterThan(0);
    expect(sent).toBeGreaterThan(targeting);
    expect(sent - targeting).toBeLessThan(2_500);
    expect(html).toContain('<option value="off">Structure seulement</option>');
    expect(html).toContain('<option value="sample" selected="">Structure et échantillon de lignes</option>');
    // The third value (TABULAR step 10): the rows the program chooses for a request to write or to analyse.
    expect(html).toContain('<option value="auto">Structure, échantillon et lignes choisies par le programme</option>');
  });

  it("puts the two folders among the main settings, whole row each, not in the advanced ones", () => {
    const html = dialog();
    const main = at(html, "Section principale");
    const documents = at(html, "Dossier des Documents");
    const tables = at(html, "Dossier des Tableaux");
    const knowledge = at(html, "Base de connaissances");
    const advanced = html.indexOf('<summary class="disclosure">Avancé</summary>');

    expect(main).toBeLessThan(documents);
    expect(documents).toBeLessThan(tables);
    expect(tables).toBeLessThan(knowledge);
    expect(advanced).toBeGreaterThan(knowledge);
    expect(html.match(/class="settings-wide settings-folder"/g)).toHaveLength(2);
  });

  it("keeps the main settings in the order of the columns", () => {
    const html = dialog();
    const order = [
      "Apparence",
      "Densité de la liste de fichiers",
      "Langue",
      "Adresse de l&#x27;IA",
      "Profil de modèle",
      "Temps d&#x27;attente",
      "Dossier des Documents",
      "Dossier des Tableaux",
      "Dossier des courriers générés",
      "Copier aussi les sources des réponses",
      "Masquer les noms dans les suggestions",
    ].map((label) => at(html, label));

    expect(order.every((position) => position >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toStrictEqual(order);
  });

  it("keeps the knowledge base in the order of the columns, with the legal module announced", () => {
    const html = dialog();
    const order = [
      "Ciblage intelligent",
      "Gérer les connaissances",
      "Module santé",
      "Module juridique",
    ].map((label) => at(html, label));

    expect(order.every((position) => position >= 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toStrictEqual(order);
    expect(html).toContain("Prévu prochainement");
  });

  it("shows the voice settings in the page, and keeps only the advanced ones in a disclosure", () => {
    const html = dialog();

    expect(html).toContain('field__label">Utiliser la voix<');
    expect(html).toContain('field__label">Vitesse de lecture<');
    // The disclosures: the advanced settings and the voice credits, nothing else.
    expect(html.match(/<details/g)).toHaveLength(2);
    expect(html).toContain('<details class="field settings-wide settings-advanced">');
  });

  it("ends with the layout and the reset, then the place of the file", () => {
    const html = dialog();

    expect(at(html, "Disposition des zones")).toBeGreaterThan(at(html, "Gestion de la voix"));
    expect(at(html, "Réinitialiser les réglages")).toBeGreaterThan(at(html, "Disposition des zones"));
    expect(html).toContain('<p class="dialog__note settings-wide">');
  });

  it("reads in English with its own catalogue", () => {
    const html = dialog("en-US");

    expect(html).toContain(">Main settings<");
    expect(html).toContain(">Knowledge base<");
    expect(html).toContain(">Voice settings<");
    expect(html).toContain(">Documents folder<");
    expect(html).toContain(">Tables folder<");
  });
});
