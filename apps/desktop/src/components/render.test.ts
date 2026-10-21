import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import type { Chip } from "../lib/entityPicker";
import type {
  EntityCard,
  EntityChoice,
  OutsideSelection,
  RetrievalDiagnostics,
  TargetingReport,
} from "../lib/ipc";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { EntityChips } from "./EntityChips";
import { EntityCardSheet } from "./EntityCardDialog";
import { EntityPicker } from "./EntityPicker";
import { SuggestionList } from "./SuggestionChips";
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
    const html = page("fr-FR", createElement(EntityChips, { chips, onRemove: () => {}, onOpenCard: () => {} }));

    expect(html).toContain("Jean Dupont");
    expect(html).toContain("Claire Martin");
    expect(html).toContain('aria-label="Retirer Jean Dupont"');
  });

  it("gives each name a button that opens its sheet, without choosing or removing it", () => {
    const html = page(
      "fr-FR",
      createElement(EntityChips, { chips, onRemove: () => {}, onOpenCard: () => {} }),
    );

    expect(html).toContain('aria-label="Voir la fiche de Jean Dupont"');
    expect(html).toContain('aria-label="Voir la fiche de Claire Martin"');
    expect(html).toContain('aria-label="Retirer Claire Martin"');
  });

  it("draws nothing without a chip", () => {
    expect(page("fr-FR", createElement(EntityChips, { chips: [], onRemove: () => {}, onOpenCard: () => {} }))).toBe("");
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
        onOpenCard: () => {},
        ...overrides,
      }),
    );
  }

  it("gives each name of the list a button for its sheet, outside the keyboard order", () => {
    const html = list();

    expect(html).toContain('aria-label="Voir la fiche de Jean Dupont"');
    expect(html).toContain('aria-label="Voir la fiche de Cabinet des Lilas"');
    expect(html.match(/class="picker__card" tabindex="-1"/g)).toHaveLength(2);
  });

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


describe("the questions worth asking, drawn", () => {
  const items = [
    { key: "a", text: "Combien de lignes pour Alice Archer ?", suggestion: {} as never },
    { key: "b", text: "Résume chaque document.", suggestion: {} as never },
  ];

  function list(overrides: Partial<Parameters<typeof SuggestionList>[0]["view"]> = {}, disabled = false): string {
    return page(
      "fr-FR",
      createElement(SuggestionList, {
        view: { items, canRevealNames: false, canHideNames: false, ...overrides },
        disabled,
        compact: false,
        onAsk: () => {},
        onRevealNamesChange: () => {},
      }),
    );
  }

  it("draws each question as a button with its title and its reassurance", () => {
    const html = list();

    expect(html).toContain("Suggestions");
    expect(html).toContain("Questions suggérées à partir de vos fichiers");
    expect(html).toContain("Combien de lignes pour Alice Archer ?");
    expect(html).toContain("Résume chaque document.");
    expect(html.match(/class="suggestions__item"/g)).toHaveLength(2);
  });

  it("locks the buttons while a question is being answered", () => {
    expect(list({}, true).match(/disabled=""/g)).toHaveLength(2);
  });

  it("offers the names only as an action of the user's own, with the reason they are hidden", () => {
    const html = list({ canRevealNames: true });

    expect(html).toContain("Les noms sont masqués");
    expect(html).toContain("Afficher les suggestions avec des noms");
    expect(html).not.toContain("Masquer à nouveau");
  });

  it("offers to hide them again once shown", () => {
    const html = list({ canHideNames: true });

    expect(html).toContain("Masquer à nouveau les noms");
    expect(html).not.toContain("Afficher les suggestions avec des noms");
  });

  it("draws nothing when there is nothing to ask and nothing to reveal", () => {
    expect(list({ items: [] })).toBe("");
  });

  it("keeps the intro out of the compact form under a summary", () => {
    const html = page(
      "fr-FR",
      createElement(SuggestionList, {
        view: { items, canRevealNames: false, canHideNames: false },
        disabled: false,
        compact: true,
        onAsk: () => {},
        onRevealNamesChange: () => {},
      }),
    );

    expect(html).toContain("suggestions--compact");
    expect(html).not.toContain("Questions suggérées à partir de vos fichiers");
  });
});

describe("the sheet of a name, drawn", () => {
  const card: EntityCard = {
    entityId: 1,
    display: "Alice Archer",
    typeId: "person",
    subtype: null,
    status: "active",
    aliases: ["Archer Alice"],
    roles: [{ role: "client", mentions: 2 }],
    documents: [{ relativePath: "archer-courrier.txt", mentions: 1, pages: [1] }],
    tables: [{ relativePath: "rdv.csv", sheet: "rdv", column: "patient", rows: 2 }],
    outside: { documents: 3, workbooks: 0 },
    relations: [],
    attributes: [],
  };

  function sheet(value: EntityCard | null | undefined, locale = "fr-FR"): string {
    return page(
      locale,
      createElement(EntityCardSheet, { card: value, display: "Alice Archer", onClose: () => {} }),
    );
  }

  it("shows the files of the selection and only counts the others", () => {
    const html = sheet(card);

    expect(html).toContain("Fiche de Alice Archer");
    expect(html).toContain("archer-courrier.txt : 1 mention, page 1");
    expect(html).toContain("rdv.csv, feuille rdv, colonne patient : 2 lignes");
    expect(html).toContain("Hors de la sélection : 3 dans les documents.");
    expect(html).toContain("Archer Alice");
    expect(html).toContain('role="dialog"');
  });

  it("uses its own class names, never a generic `card` (a shared one squashed the settings)", () => {
    const html = sheet(card);

    expect(html).toContain('class="entity-card"');
    expect(html).not.toMatch(/class="card[ "_]/);
    expect(html).not.toMatch(/class="[^"]*card__/);
  });

  it("says that it is being read, and that a name is gone", () => {
    expect(sheet(undefined)).toContain("Lecture de la fiche");
    expect(sheet(null)).toContain("existe plus");
    expect(sheet(null)).toContain("Fiche de Alice Archer");
  });

  it("says when the selection holds the name nowhere", () => {
    const html = sheet({ ...card, documents: [], tables: [] });

    expect(html).toContain("apparaît dans aucun fichier de la sélection actuelle");
  });

  it("is drawn in English too", () => {
    const html = sheet(card, "en-US");

    expect(html).toContain("Sheet of Alice Archer");
    expect(html).toContain("Outside the selection: 3 in documents.");
    expect(html).toContain("Close");
  });
});
