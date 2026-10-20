import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { I18nProvider } from "../i18n/I18nProvider";
import { createTranslator } from "../i18n/translate";
import { endLine, runLine } from "../lib/analysisLines";
import { progressView, traceOf, type ProgressSample } from "../lib/analysisProgress";
import { countsByFamily } from "../lib/explorerModel";
import type { IndexProgress, IndexSummary } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";
import { ExplorerFamilySection } from "./ExplorerFamilySection";

/* The gauge, the line under the header and the end of a pass, drawn to static markup with the real
   catalogues. The rates and the gates are tested where they live (`lib/analysisProgress.test.ts`);
   here only what a given state looks like. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

const NOOP = () => undefined;

/** French separates thousands with a narrow no-break space: compared as an ordinary one. */
function plain(text: string): string {
  return text.replace(/[  ]/g, " ").replace(/&#x27;/g, "'");
}

function page(locale: string, element: ReactElement): string {
  return plain(renderToStaticMarkup(createElement(I18nProvider, { locale, children: element })));
}

function reading(done: number, total: number): IndexProgress {
  return { processedFiles: done, totalFiles: total, batchIndex: 0, batchTotal: 0 };
}

/** `perSecond` files a second out of `total`, one message a second, up to `seconds`. */
function steady(total: number, perSecond: number, seconds: number): ProgressSample[] {
  return [...Array(seconds + 1).keys()].map((second) => ({
    at: second * 1000,
    progress: reading(Math.min(total, second * perSecond), total),
  }));
}

const IDLE: IndexingState = {
  running: false,
  progress: null,
  trace: null,
  durationMs: null,
  summary: null,
  error: null,
  finishedPasses: 0,
  run: async () => true,
  reset: () => undefined,
};

function running(samples: ProgressSample[], startedAt = 0): IndexingState {
  return { ...IDLE, running: true, trace: traceOf(startedAt, samples) };
}

function section(
  indexing: IndexingState,
  now: number,
  family: "documents" | "tables" = "documents",
  locale = "fr-FR",
  overrides: Record<string, unknown> = {},
) {
  return page(
    locale,
    createElement(ExplorerFamilySection, {
      family,
      folder: "/some/folder",
      isEmpty: false,
      folderActions: { reveal: NOOP, change: NOOP, clear: NOOP },
      counts: countsByFamily([])[family],
      entries: [],
      allEntries: [],
      open: true,
      onToggleOpen: NOOP,
      rows: 6,
      rowHeight: 24,
      indexing,
      now,
      onStop: NOOP,
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

describe("a family while its pass runs", () => {
  it("replaces Analyser by the gauge with its percentage and a Stop, and drops the menu", () => {
    const html = section(running(steady(10, 1, 4)), 4000);

    expect(html).toContain('class="gauge"');
    expect(html).toContain('role="status"');
    expect(html).toContain("Lecture 40 %");
    expect(html).toContain('aria-label="Arrêter l\'analyse : Documents"');
    expect(html).not.toContain(">Analyser");
    expect(html).not.toContain("explorer-family__menu");
    expect(html).toContain("explorer-family__fold");
    expect(html.indexOf("gauge__stop")).toBeLessThan(html.indexOf("explorer-family__fold"));
  });

  it("is not a disabled button, so the label stays readable", () => {
    const html = section(running(steady(10, 1, 4)), 4000);
    const body = html.slice(html.indexOf("gauge__body"), html.indexOf('<button type="button" class="gauge__stop"'));

    expect(body).not.toContain("<button");
    expect(body).not.toContain("disabled");
  });

  it("says the stage, the files and the times on one line, estimating at first", () => {
    const html = section(running(steady(1245, 3, 4)), 4000);

    expect(html).toContain(
      "Étape 1 sur 2 · Lecture des documents · 12 / 1 245 · 4 s écoulées · estimation en cours…",
    );
  });

  it("gives a remaining time once 10 % and 8 s are behind it", () => {
    const html = section(running(steady(200, 2, 30)), 30_000);

    expect(html).toContain("Lecture 30 %");
    expect(html).toMatch(/· 30 s écoulées · environ 1 min restante</);
    expect(html).not.toContain("estimation en cours");
  });

  it("counts the minutes elapsed", () => {
    const html = section(running(steady(2000, 1, 228)), 228_400);

    expect(html).toContain("3 min 48 s écoulées");
  });

  it("names the tables' single stage with no step, and its own wording", () => {
    const html = section(running(steady(50, 1, 22)), 22_000, "tables");

    expect(html).toContain("Lecture 44 %");
    expect(html).toContain("Lecture des tableaux · 22 / 50");
    expect(html).not.toContain("Étape");
    expect(html).toContain('aria-label="Arrêter l\'analyse : Tableaux"');
  });

  it("says in the second stage that the word search already works", () => {
    const meaning: ProgressSample = {
      at: 5000,
      progress: { ...reading(40, 40), vectorsDone: 2500, vectorsTotal: 10_000 },
    };
    const html = section(running([meaning]), 5000);

    expect(html).toContain("Sens 25 %");
    expect(html).toContain("Étape 2 sur 2 · Recherche par sens · 2 500 / 10 000 passages");
    expect(html).toContain("La recherche par mots fonctionne déjà.");
  });

  it("starts with a gauge at zero before the pass has said anything", () => {
    const html = section({ ...IDLE, running: true, trace: traceOf(0, []) }, 2000);

    expect(html).toContain("Démarrage…");
    expect(html).toContain("2 s écoulées");
  });

  it("hides the notes about the last index while the pass runs", () => {
    const html = section(running(steady(10, 1, 4)), 4000, "documents", "fr-FR", {
      needsAnalyse: true,
      meaningPercent: 0,
    });

    expect(html).not.toContain("L'index doit être refait");
    expect(html).not.toContain("Recherche par sens : 0 %");
    const idle = section(IDLE, 0, "documents", "fr-FR", { needsAnalyse: true, meaningPercent: 0 });
    expect(idle).toContain("L'index doit être refait");
  });

  it("draws the thin line under the header, filled to the percentage", () => {
    const html = section(running(steady(10, 1, 4)), 4000);

    expect(html).toContain('class="explorer-family__line"');
    expect(html).toContain('style="width:40%"');
  });

  it("writes the same line in English", () => {
    const html = section(running(steady(200, 2, 30)), 30_000, "documents", "en-US");

    expect(html).toContain("Reading 30 %");
    expect(html).toContain("Step 1 of 2 · Reading the documents · 60 / 200 · 30 s elapsed · about 1 min left");
    expect(html).toContain("Stop the analysis: Documents");
  });
});

describe("a family after its pass", () => {
  it("says what was done, how many could not be read and how long it took", () => {
    const html = section(
      {
        ...IDLE,
        durationMs: 409_000,
        summary: summary({
          indexedFiles: 1245,
          emptyFiles: ["a.pdf", "b.pdf"],
          failedFiles: [{ path: "c.pdf", code: "x", data: {} }],
        }),
      },
      0,
    );

    expect(html).toContain("Terminé · 1 245 analysés, 3 illisibles · 6 min 49 s");
    expect(html).not.toContain('class="gauge"');
    expect(html).toContain(">Analyser");
    expect(html).not.toContain("Voir le rapport");
  });

  it("says a stopped pass is stopped and what is kept", () => {
    const html = section(
      { ...IDLE, durationMs: 228_000, summary: summary({ cancelled: true, indexedFiles: 400 }) },
      0,
    );

    expect(html).toContain("Analyse arrêtée · 3 min 48 s · ce qui est lu est conservé");
    expect(html).not.toContain("Terminé");
  });

  it("says when there was nothing new", () => {
    const html = section({ ...IDLE, durationMs: 2000, summary: summary({ unchangedFiles: 12 }) }, 0);

    expect(html).toContain("Terminé · rien de nouveau à analyser · 2 s");
  });

  it("draws no line at all before any pass", () => {
    expect(section(IDLE, 0)).not.toContain("explorer-family__run");
  });

  it("counts one file in the singular", () => {
    const t = createTranslator(CATALOGUES["fr-FR"] ?? {}, CATALOGUES[REFERENCE_LOCALE] ?? {});

    expect(endLine(t, "fr-FR", summary({ indexedFiles: 1, emptyFiles: ["a"] }), 5000)).toBe(
      "Terminé · 1 analysé, 1 illisible · 5 s",
    );
  });
});

describe("the line under the header", () => {
  it("puts the times last, so that an ellipsis cuts them first", () => {
    const t = createTranslator(CATALOGUES["fr-FR"] ?? {}, CATALOGUES[REFERENCE_LOCALE] ?? {});
    const trace = traceOf(0, steady(200, 2, 30));
    const view = progressView(trace, 30_000, "documents");
    const line = plain(runLine(t, "fr-FR", "documents", view, 30_000));
    const parts = line.split(" · ");

    expect(parts).toHaveLength(5);
    expect(parts[3]).toContain("écoulées");
    expect(parts[4]).toContain("restante");
  });
});
