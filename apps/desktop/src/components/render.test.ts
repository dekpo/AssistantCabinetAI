import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import type { Chip } from "../lib/entityPicker";
import type {
  EntityChoice,
  OutsideSelection,
  RetrievalDiagnostics,
  TargetingReport,
} from "../lib/ipc";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { EntityChips } from "./EntityChips";
import { EntityPicker } from "./EntityPicker";
import { TargetingNote } from "./TargetingNote";

/* The window is not opened in these tests: each component is drawn to static markup, with the real
   catalogues, to read the words and the buttons the user would see. They do not click; what a click
   asks for is covered where the logic lives (`lib/targeting.ts`, `lib/entityPicker.ts`). */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  // The provider sets the document language; there is no document in this environment.
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(locale: string, element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

const report: TargetingReport = {
  strategy: "entity_files",
  reason: null,
  requestedBy: "auto",
  widened: "none",
  selectedDocuments: 30,
  candidateDocuments: 4,
  selectedWorkbooks: 0,
  candidateWorkbooks: 0,
  reductionPercent: 86,
  entities: [
    { entityId: 7, display: "Jean Dupont", typeId: "person", confidence: 0.95, method: "alias" },
  ],
  candidatePaths: [
    "dupont-bilan-2026.txt",
    "dupont-horaires.txt",
    "dupont-lettre-confrere.txt",
    "rappel-m-dupont.txt",
  ],
  workbookPaths: [],
  ambiguities: [],
  possibles: [],
  chosenWorkbook: null,
};

function note(locale: string, overrides: Partial<TargetingReport>, idle = true): string {
  return page(
    locale,
    createElement(TargetingNote, {
      report: { ...report, ...overrides },
      chosenIds: [],
      idle,
      onChoose: () => {},
    }),
  );
}

describe("the line under an answer, drawn", () => {
  it("shows the sentence and both buttons", () => {
    const html = note("fr-FR", {});

    expect(html).toContain("Recherche ciblée : 4 documents sur 30 — Jean Dupont");
    expect(html).toContain("Élargir à ma sélection");
    expect(html).toContain("Voir les fichiers");
    // The files are not listed until she asks.
    expect(html).not.toContain("dupont-bilan-2026.txt");
  });

  it("is in her language", () => {
    const html = note("en-US", {});

    expect(html).toContain("Targeted search: 4 documents out of 30 — Jean Dupont");
    expect(html).toContain("Widen to my selection");
    expect(html).not.toContain("Recherche");
  });

  it("offers no button while an answer is being written", () => {
    const html = note("fr-FR", {}, false);

    expect(html).toContain("Recherche ciblée");
    expect(html).not.toContain("Élargir à ma sélection");
  });

  it("draws nothing for a search that was not narrowed", () => {
    expect(note("fr-FR", { candidatePaths: [] })).toBe("");
  });

  it("names the workbook chosen and offers another", () => {
    const html = note("fr-FR", {
      candidatePaths: [],
      workbookPaths: ["factures-2026.xlsx"],
      selectedWorkbooks: 3,
      candidateWorkbooks: 1,
      chosenWorkbook: { file: "factures-2026.xlsx", reason: "entity_files" },
    });

    expect(html).toContain("Classeur choisi : factures-2026.xlsx — Jean Dupont y figure");
    expect(html).toContain("Choisir un autre classeur");
    expect(html).not.toContain("Voir les fichiers");
  });

  it("says the search was widened by itself, with no widen button", () => {
    const html = note("fr-FR", { widened: "auto" });

    expect(html).toContain("Recherche élargie à toute votre sélection");
    expect(html).not.toContain("Élargir à ma sélection");
  });

  it("asks which person was meant, with a button for each name of the selection", () => {
    const html = page(
      "fr-FR",
      createElement(TargetingNote, {
        report: {
          ...report,
          candidatePaths: [],
          ambiguities: [
            {
              span: "Dr Martin",
              candidates: [
                { entityId: 1, display: "Claire Martin", typeId: "person", documents: 2, workbooks: 0 },
                { entityId: 2, display: "Paul Martin", typeId: "person", documents: 1, workbooks: 0 },
                { entityId: 3, display: "Marc Martin", typeId: "person", documents: 0, workbooks: 0 },
              ],
            },
          ],
        },
        chosenIds: [],
        idle: true,
        onChoose: () => {},
      }),
    );

    expect(html).toContain("Plusieurs personnes correspondent à « Dr Martin » :");
    expect(html).toContain("Claire Martin (2 fichiers)");
    expect(html).toContain("Paul Martin (1 fichier)");
    // Not in the selection: never drawn.
    expect(html).not.toContain("Marc Martin");
    // Nothing is asked while an answer is being written.
    expect(
      page(
        "fr-FR",
        createElement(TargetingNote, {
          report: { ...report, candidatePaths: [] },
          chosenIds: [],
          idle: false,
          onChoose: () => {},
        }),
      ),
    ).toBe("");
  });
});

describe("the names chosen for the next question, drawn", () => {
  const chips: Chip[] = [
    { entityId: 7, display: "Jean Dupont" },
    { entityId: 8, display: "Claire Martin" },
  ];

  it("shows each name with a button to remove it", () => {
    const html = page("fr-FR", createElement(EntityChips, { chips, onRemove: () => {} }));

    expect(html).toContain("Jean Dupont");
    expect(html).toContain("Claire Martin");
    expect(html).toContain('aria-label="Retirer Jean Dupont"');
  });

  it("draws nothing without a chip", () => {
    expect(page("fr-FR", createElement(EntityChips, { chips: [], onRemove: () => {} }))).toBe("");
  });
});

describe("the list of names, drawn", () => {
  const choices: EntityChoice[] = [
    { entityId: 7, display: "Jean Dupont", typeId: "person", documents: 3, workbooks: 0 },
    { entityId: 9, display: "Cabinet des Lilas", typeId: "organization", documents: 1, workbooks: 1 },
  ];
  const outside: OutsideSelection = {
    entityId: 11,
    display: "Jean Dupont",
    typeId: "person",
    documents: ["a.txt", "b.txt", "c.txt"],
    workbooks: ["rdv.csv"],
  };

  function list(overrides: Partial<Parameters<typeof EntityPicker>[0]> = {}): string {
    return page(
      "fr-FR",
      createElement(EntityPicker, {
        id: "picker",
        prefix: "Du",
        choices,
        outside: [],
        highlighted: 1,
        searching: false,
        onChoose: () => {},
        onHighlight: () => {},
        onAddToSelection: () => {},
        ...overrides,
      }),
    );
  }

  it("lists the names with their kind and their number of files, and marks the highlighted one", () => {
    const html = list();

    expect(html).toContain("Jean Dupont");
    expect(html).toContain("Personne · 3 fichiers");
    expect(html).toContain("Organisation · 2 fichiers");
    expect(html).toContain('role="listbox"');
    expect(html).toMatch(/id="picker-1"[^>]*aria-selected="true"/);
    expect(html).toMatch(/id="picker-0"[^>]*aria-selected="false"/);
  });

  it("says what to do when nothing was typed yet, and when nothing matches", () => {
    expect(list({ choices: [], prefix: "" })).toContain("Tapez les premières lettres d&#x27;un nom.");
    expect(list({ choices: [], searching: true })).toContain("Recherche…");
    expect(list({ choices: [] })).toContain("Aucun nom de votre sélection ne commence ainsi.");
  });

  it("sets the outside-selection line apart, labelled, with its own button", () => {
    const html = list({ choices: [], outside: [outside] });

    expect(html).toContain("Jean Dupont figure dans 4 fichiers non sélectionnés");
    expect(html).toContain("Ajouter à ma sélection");
    expect(html).toContain("picker__outside");
  });
});

describe("the developer panel, drawn", () => {
  it("is closed, titled, and says so when there is nothing to show", () => {
    const html = page("fr-FR", createElement(DiagnosticsPanel, { diagnostics: undefined }));

    expect(html).toContain("<details");
    expect(html).toContain("Diagnostics (développeur)");
    expect(html).toContain("Aucune mesure pour cette réponse.");
  });

  it("lists this answer's rows", () => {
    const measured = {
      path: "documents",
      plan: "whole_folder",
      selectedDocuments: 30,
      selectedWorkbooks: 0,
      chunksConsidered: 120,
      chunksSelected: 6,
      estimatedInputChars: 5400,
      inventoryMs: 12,
      scopeMs: 1,
      routingMs: 3,
      embeddingMs: 210,
      searchMs: 9,
      workbookLoadMs: 0,
      workbookCacheHit: false,
      generationFirstTokenMs: 4100,
      generationTotalMs: 23000,
      totalMs: 23500,
      targeting: null,
    } satisfies RetrievalDiagnostics;

    const html = page("en-US", createElement(DiagnosticsPanel, { diagnostics: measured }));

    expect(html).toContain("30 documents, 0 workbooks");
    expect(html).toContain("23500 ms");
  });
});
