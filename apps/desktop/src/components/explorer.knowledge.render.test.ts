import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { joinFacts } from "../lib/explorerFacts";
import { NO_CHOICES } from "../lib/explorerFilters";
import { NO_VIEW } from "../lib/explorerView";
import { savedFromView } from "../lib/explorerViews";
import type { FolderKnowledgeFacts } from "../lib/ipc";
import { ExplorerActivePills } from "./ExplorerActivePills";
import { ExplorerAutoSwitch } from "./ExplorerAutoSwitch";
import { ExplorerBottomBar } from "./ExplorerBottomBar";
import { ExplorerKnowledgeFilters } from "./ExplorerKnowledgeFilters";
import { ExplorerViewsPanel } from "./ExplorerViewsPanel";

/* The pieces added by the explorer's knowledge-base step, drawn to static markup with the real
   catalogues. What a click asks for lives in the pure modules, tested where the logic is. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(locale: string, element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

const NOOP = () => undefined;

const documents: FolderKnowledgeFacts = {
  files: [],
  entities: [
    { entityId: 1, name: "Alice Martin", typeId: "person" },
    { entityId: 10, name: "Acme SA", typeId: "organization" },
  ],
};
const facts = joinFacts({ documents, data: null });

describe("the Select. auto switch", () => {
  it("is a switch, off, with its hint line", () => {
    const html = page("fr-FR", createElement(ExplorerAutoSwitch, { on: false, disabled: false, onToggle: NOOP }));

    expect(html).toMatch(/<button[^>]*role="switch"[^>]*aria-checked="false"/);
    expect(html).toContain("Select. auto");
    expect(html).toContain("La sélection suit les filtres.");
    expect(html).not.toContain('disabled=""');
  });

  it("says it is on and is locked while a question is being answered", () => {
    const html = page("fr-FR", createElement(ExplorerAutoSwitch, { on: true, disabled: true, onToggle: NOOP }));

    expect(html).toMatch(/aria-checked="true"/);
    expect(html).toContain('disabled=""');
  });

  it("reads in English too", () => {
    const html = page("en-US", createElement(ExplorerAutoSwitch, { on: false, disabled: false, onToggle: NOOP }));

    expect(html).toContain("Auto select");
  });
});

describe("the bottom bar and the mode", () => {
  const bar = (auto: Parameters<typeof ExplorerBottomBar>[0]["auto"]) =>
    page(
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
        auto,
      }),
    );

  it("always shows the mode, manual first", () => {
    expect(bar({ on: false, unreachable: 0, restore: null, onRestore: NOOP })).toContain("Mode : manuel");
    expect(bar({ on: true, unreachable: 0, restore: null, onRestore: NOOP })).toContain(
      "Mode : automatique (suit les filtres)",
    );
  });

  it("offers the way back to the previous selection while a snapshot exists", () => {
    const html = bar({ on: false, unreachable: 0, restore: 3, onRestore: NOOP });

    expect(html).toContain("Rétablir la sélection précédente (3)");
    expect(bar({ on: false, unreachable: 0, restore: null, onRestore: NOOP })).not.toContain("Rétablir");
  });

  it("says how many results cannot be ticked, only while it is on", () => {
    expect(bar({ on: true, unreachable: 1, restore: null, onRestore: NOOP })).toContain(
      "1 résultat ne peut pas être coché : il n&#x27;est pas analysé.",
    );
    expect(bar({ on: true, unreachable: 4, restore: null, onRestore: NOOP })).toContain(
      "4 résultats ne peuvent pas être cochés",
    );
    expect(bar({ on: false, unreachable: 4, restore: null, onRestore: NOOP })).not.toContain("ne peuvent pas");
    expect(bar({ on: true, unreachable: 0, restore: null, onRestore: NOOP })).not.toContain("ne peut pas");
  });

  it("draws no mode where the switch does not exist", () => {
    expect(bar(undefined)).not.toContain("Mode :");
  });
});

describe("the knowledge-base filters of the panel", () => {
  const filters = (over: Partial<Parameters<typeof ExplorerKnowledgeFilters>[0]> = {}) =>
    page(
      "fr-FR",
      createElement(ExplorerKnowledgeFilters, {
        loaded: true,
        types: [
          { code: "invoice", count: 3 },
          { code: "my_own_type", count: 1 },
        ],
        chosenTypes: new Set(["invoice"]),
        years: [
          { year: "2026", count: 4 },
          { year: "2025", count: 0 },
        ],
        chosenYears: new Set<string>(),
        people: [
          {
            entityId: 1,
            name: "Alice Martin",
            count: 4,
            roles: [
              { role: "author", count: 2 },
              { role: "subject", count: 1 },
              { role: "mention", count: 1 },
            ],
          },
        ],
        chosenPeople: new Set(["1"]),
        organisations: [{ entityId: 10, name: "Acme SA", count: 2, roles: [] }],
        chosenOrganisations: new Set<string>(),
        disabled: false,
        onToggle: NOOP,
        onRole: NOOP,
        ...over,
      }),
    );

  it("lists the business types by their name in the user's language with their counts", () => {
    const html = filters();

    expect(html).toContain("Type de document");
    expect(html).toMatch(/aria-pressed="true"[^>]*>Facture<small[^>]*>3</);
    // A type the catalogue has no word for keeps its code rather than showing a gap.
    expect(html).toContain("my_own_type");
    expect(html).not.toContain("businessType.");
  });

  it("lists the years with their counts and lights a year with no file in grey", () => {
    const html = filters();

    expect(html).toContain("Année");
    expect(html).toMatch(/>2026<small[^>]*>4</);
    expect(html).toMatch(/data-empty="true"[^>]*>2025<small[^>]*>0</);
  });

  it("draws a typed field for people and another for organisations, with no list until focused", () => {
    const html = filters();

    expect(html).toContain("Personnes");
    expect(html).toContain("Organisations");
    expect(html).toContain('placeholder="Tapez un nom"');
    expect(html).not.toContain("explorer-suggest");
  });

  it("shows a chosen name as a chip with the role step: any role, then the roles it has", () => {
    const html = filters();

    expect(html).toMatch(/aria-pressed="true"[^>]*>Alice Martin<small[^>]*>4</);
    expect(html).toContain("Tous les rôles");
    expect(html).toMatch(/>Auteur<small[^>]*>2</);
    expect(html).toMatch(/>Concerné<small[^>]*>1</);
    // A name met in the text with no role is not a role to choose.
    expect(html).not.toContain(">mention<");
    expect(html).not.toContain("Cité<small");
  });

  it("marks the role that was chosen", () => {
    const html = filters({ chosenPeople: new Set(["1:author"]) });

    expect(html).toMatch(/aria-pressed="true"[^>]*>Auteur</);
    expect(html).toMatch(/aria-pressed="false"[^>]*>Tous les rôles</);
  });

  it("says a chosen name the base no longer knows is not found, and lets it be removed", () => {
    const html = filters({
      people: [{ entityId: 99, name: null, count: 0, roles: [] }],
      chosenPeople: new Set(["99"]),
    });

    expect(html).toContain("Nom introuvable");
    expect(html).toContain("Retirer ce nom");
  });

  it("says it is reading, then that there is nothing to offer", () => {
    expect(filters({ loaded: false })).toContain("Lecture de la base de connaissances");
    const none = filters({ types: [], years: [], people: [], organisations: [], chosenPeople: new Set<string>() });
    expect(none).toContain("lancez Analyser");
    expect(none).not.toContain("Personnes");
  });
});

describe("the pills of the knowledge base", () => {
  const pills = (kinds: Parameters<typeof ExplorerActivePills>[0]["pills"], loaded = true) =>
    page(
      "fr-FR",
      createElement(ExplorerActivePills, {
        pills: kinds,
        folders: [],
        facts,
        factsLoaded: loaded,
        onRemove: NOOP,
        onClear: NOOP,
      }),
    );

  it("name the person with the role, the organisation, the type, the year and the two switches", () => {
    const html = pills([
      { kind: "person", value: "1:author", typed: false },
      { kind: "organisation", value: "10", typed: false },
      { kind: "type", value: "invoice", typed: false },
      { kind: "year", value: "2026", typed: false },
      { kind: "conversation", value: "conversation", typed: false },
      { kind: "recent", value: "recent", typed: false },
    ]);

    expect(html).toContain("Alice Martin · Auteur");
    expect(html).toContain("Acme SA");
    expect(html).toContain("Facture");
    expect(html).toContain("2026");
    expect(html).toContain("Dans la conversation");
    expect(html).toContain("Plus récents d&#x27;abord");
  });

  it("say a name that is gone is not found once the facts are read, and wait before", () => {
    const gone = [{ kind: "person" as const, value: "99", typed: false }];

    expect(pills(gone, true)).toContain("Nom introuvable");
    expect(pills(gone, false)).not.toContain("Nom introuvable");
  });

  it("say a folder that no longer exists is not found", () => {
    const html = page(
      "fr-FR",
      createElement(ExplorerActivePills, {
        pills: [{ kind: "folder", value: "documents:Old", typed: false }],
        folders: [],
        foldersKnown: true,
        onRemove: NOOP,
        onClear: NOOP,
      }),
    );

    expect(html).toContain("Old (introuvable)");
  });
});

describe("the views", () => {
  const views = (over: Partial<Parameters<typeof ExplorerViewsPanel>[0]> = {}) =>
    page(
      "fr-FR",
      createElement(ExplorerViewsPanel, {
        saved: [],
        current: NO_VIEW,
        facts,
        factsLoaded: true,
        onOpen: NOOP,
        onSave: NOOP,
        onRename: NOOP,
        onDelete: NOOP,
        ...over,
      }),
    );

  it("lists the three views of the application", () => {
    const html = views();

    expect(html).toContain("Vues de l&#x27;application");
    for (const name of ["Récents", "À analyser", "Dans la conversation"]) {
      expect(html).toContain(name);
    }
  });

  it("says there is no saved view, and that nothing can be saved without a filter", () => {
    const html = views();

    expect(html).toContain("Aucune vue enregistrée.");
    expect(html).toContain("Aucun filtre à enregistrer.");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Enregistrer<\/button>/);
  });

  it("lists the saved views with open, rename and delete", () => {
    const current = { ...NO_VIEW, choices: { ...NO_CHOICES, years: new Set(["2026"]) } };
    const saved = [savedFromView(current, "v1", "Mes factures")];
    const html = views({ saved, current });

    expect(html).toContain("Mes factures");
    expect(html).toContain("Renommer");
    expect(html).toContain("Supprimer");
    // The view that is open is lit.
    expect(html).toMatch(/aria-pressed="true"[^>]*title="Mes factures"/);
    expect(html).not.toContain("Aucune vue enregistrée.");
  });

  it("says a view names someone the files no longer link to", () => {
    const withGhost = {
      ...NO_VIEW,
      choices: { ...NO_CHOICES, people: new Set(["1", "99"]) },
    };
    const html = views({ saved: [savedFromView(withGhost, "v1", "Fantôme")] });

    expect(html).toContain("Un nom de ce filtre ne figure plus dans vos fichiers.");
  });

  it("does not call a name missing before the facts have been read", () => {
    const withGhost = { ...NO_VIEW, choices: { ...NO_CHOICES, people: new Set(["99"]) } };
    const html = views({ saved: [savedFromView(withGhost, "v1", "Fantôme")], factsLoaded: false });

    expect(html).not.toContain("ne figure plus");
  });
});
