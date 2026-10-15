import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../../i18n/I18nProvider";
import type {
  AliasView,
  ColumnView,
  EntityDetail,
  MentionView,
  PossibleView,
  RelationView,
} from "../../lib/ipc";
import { KnowledgeLink } from "../FolderCardParts";
import {
  AliasList,
  AttributeList,
  ColumnList,
  PossibleList,
  ProvenanceList,
  RelationList,
  SheetCounts,
} from "./EntitySheetParts";

/* The window is not opened in these tests: each section of an entity's sheet is drawn to static
   markup, with the real catalogues, to read the words and the buttons the user would see. They do
   not click; what a click asks for is covered where the logic lives (`lib/knowledgeManage.ts`) and
   what Rust is asked for in `lib/ipc.manage.test.ts`. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(locale: string, element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

function mention(overrides: Partial<MentionView>): MentionView {
  return {
    mentionId: 1,
    entityId: 7,
    locator: {
      kind: "chunk",
      page: 3,
      section: 2,
      sheet: null,
      column: null,
      firstRow: null,
      sampleRows: [],
    },
    excerpt: { text: "Le docteur Jean Dupont recommande un bilan.", cutBefore: true, cutAfter: false },
    alias: "Jean Dupont",
    role: null,
    method: "title_full_name",
    confidence: 0.85,
    origin: "automatic",
    occurrences: 1,
    splitFrom: null,
    ...overrides,
  };
}

const cellsMention = mention({
  mentionId: 2,
  locator: {
    kind: "cells",
    page: null,
    section: null,
    sheet: "Mars",
    column: "Medecin",
    firstRow: 2,
    sampleRows: [2, 5, 9],
  },
  excerpt: null,
  alias: null,
  role: "provider",
  method: "column_value",
  confidence: 0.95,
  occurrences: 4,
});

function detail(overrides: Partial<EntityDetail> = {}): EntityDetail {
  return {
    entity: {
      entityId: 7,
      typeId: "person",
      subtype: null,
      name: "Jean Dupont",
      disambiguator: "",
      status: "active",
      origin: "automatic",
      mergedInto: null,
      mentions: 2,
      sources: 2,
      aliases: 2,
      openMatches: 0,
    },
    effectiveId: 7,
    survivor: null,
    aliases: [],
    mergedIn: [],
    sources: [
      {
        sourceId: 1,
        domain: "documents",
        relativePath: "rapports/mars.txt",
        truncated: false,
        mentions: [mention({})],
      },
      { sourceId: 2, domain: "data", relativePath: "rdv.xlsx", truncated: false, mentions: [cellsMention] },
    ],
    mentionCount: 2,
    relations: [],
    attributes: [],
    possibleMatches: [],
    columns: [],
    ...overrides,
  };
}

function provenance(locale: string, overrides: Partial<EntityDetail> = {}, picking = false): string {
  return page(
    locale,
    createElement(ProvenanceList, {
      detail: detail(overrides),
      picking,
      picked: [1],
      onToggle: () => {},
    }),
  );
}

describe("where an entity comes from, drawn", () => {
  it("names the source, the place, the method and the confidence of each mention", () => {
    const html = provenance("fr-FR");

    expect(html).toContain("rapports/mars.txt");
    expect(html).toContain("page 3, passage 2");
    expect(html).toContain("titre suivi d&#x27;un nom complet");
    expect(html).toContain("85 %");
    expect(html).toContain("rdv.xlsx");
    expect(html).toContain("feuille « Mars », colonne « Medecin », lignes 2, 5, 9");
    expect(html).toContain("4 fois");
    expect(html).toContain("valeur d&#x27;une colonne reconnue");
    expect(html).toContain("95 %");
  });

  it("distinguishes a document from a spreadsheet", () => {
    const html = provenance("fr-FR");

    expect(html).toContain("Document");
    expect(html).toContain("Tableur");
  });

  it("quotes the passage around the name, and shows where it was cut", () => {
    const html = provenance("fr-FR");

    expect(html).toContain("… Le docteur Jean Dupont recommande un bilan.");
  });

  it("shows the name that matched and the role, in her language", () => {
    const html = provenance("fr-FR");

    expect(html).toContain("Nom trouvé : « Jean Dupont »");
    expect(html).toContain("Rôle : Prestataire");
  });

  it("is in English too", () => {
    const html = provenance("en-US");

    expect(html).toContain("sheet “Mars”, column “Medecin”, rows 2, 5, 9");
    expect(html).toContain("Where this entity comes from");
    expect(html).not.toContain("Tableur");
  });

  it("says when the sheet lists fewer mentions than the entity has", () => {
    const html = provenance("fr-FR", { mentionCount: 250 });

    expect(html).toContain("2 mentions affichées sur 250.");
  });

  it("says when there is no mention at all", () => {
    const html = provenance("fr-FR", { sources: [], mentionCount: 0 });

    expect(html).toContain("Aucune mention enregistrée pour le moment.");
  });

  it("shows a tick box on every mention only while a split is being prepared", () => {
    expect(provenance("fr-FR", {}, false)).not.toContain('type="checkbox"');

    const picking = provenance("fr-FR", {}, true);
    expect(picking.match(/type="checkbox"/g)?.length).toBe(2);
    // The mention already ticked is checked.
    expect(picking.match(/checked=""/g)?.length).toBe(1);
  });

  it("says what a hand-made mention is, and where a separated one came from", () => {
    const html = provenance("fr-FR", {
      sources: [
        {
          sourceId: 1,
          domain: "documents",
          relativePath: "a.txt",
          truncated: false,
          mentions: [
            mention({ origin: "manual", splitFrom: 3, entityId: 9, method: "manual" }),
          ],
        },
      ],
      mentionCount: 1,
    });

    expect(html).toContain("Saisi à la main");
    expect(html).toContain("Séparé à la main d&#x27;une autre entité");
    expect(html).toContain("Vient d&#x27;une entité fusionnée ici");
  });

  it("never shows a machine code", () => {
    const html = provenance("fr-FR");

    for (const code of ["title_full_name", "column_value", "chunk", "cells", "documents"]) {
      expect(html, code).not.toContain(code);
    }
  });
});

describe("the names of an entity, drawn", () => {
  const aliases: AliasView[] = [
    { aliasId: 1, entityId: 7, display: "Jean Dupont", kind: "canonical_variant", origin: "automatic", confidence: 1 },
    { aliasId: 2, entityId: 7, display: "Docteur JD", kind: "manual", origin: "manual", confidence: 1 },
    { aliasId: 3, entityId: 8, display: "J. Dupont", kind: "initial", origin: "automatic", confidence: 1 },
  ];

  function names(editable: boolean): string {
    return page(
      "fr-FR",
      createElement(AliasList, { aliases, ownerId: 7, editable, onRemove: () => {} }),
    );
  }

  it("lists each name with its kind", () => {
    const html = names(true);

    expect(html).toContain("Jean Dupont");
    expect(html).toContain("Forme du nom");
    expect(html).toContain("Ajouté à la main");
    expect(html).toContain("Initiale");
  });

  it("offers to remove a name typed by hand and no other", () => {
    const html = names(true);

    expect(html.match(/Retirer ce nom/g)?.length).toBe(1);
  });

  it("offers nothing to remove on an entity that cannot be edited", () => {
    expect(names(false)).not.toContain("Retirer ce nom");
  });

  it("marks a name that comes from an entity merged into this one", () => {
    expect(names(true)).toContain("Vient d&#x27;une entité fusionnée ici");
  });
});

describe("the other sections, drawn", () => {
  it("draws the details, with the pronunciation labelled", () => {
    const html = page(
      "fr-FR",
      createElement(AttributeList, {
        attributes: [{ key: "spoken_form", value: "docteur Dupont", origin: "manual" }],
        editable: true,
        onRemove: () => {},
      }),
    );

    expect(html).toContain("Prononciation");
    expect(html).toContain("docteur Dupont");
    expect(html).toContain("Retirer cette précision");
  });

  it("draws nothing for an entity without details, links, pairs or columns", () => {
    expect(page("fr-FR", createElement(AttributeList, { attributes: [], editable: true, onRemove: () => {} }))).toBe("");
    expect(page("fr-FR", createElement(RelationList, { relations: [] }))).toBe("");
    expect(page("fr-FR", createElement(PossibleList, { matches: [], onOpen: () => {} }))).toBe("");
    expect(page("fr-FR", createElement(ColumnList, { workbooks: [] }))).toBe("");
  });

  it("draws a link read between two columns in words", () => {
    const relation: RelationView = {
      relationId: 1,
      predicate: "served_by",
      subject: { entityId: 1, typeId: "person", name: "Alice Exemple", status: "active" },
      object: { entityId: 2, typeId: "person", name: "Jean Dupont", status: "active" },
      relativePath: "rdv.xlsx",
      occurrences: 3,
    };

    const html = page("fr-FR", createElement(RelationList, { relations: [relation] }));

    expect(html).toContain("Alice Exemple");
    expect(html).toContain("suivi par");
    expect(html).toContain("Jean Dupont");
    expect(html).not.toContain("served_by");
  });

  it("draws a pair with its state and its reason", () => {
    const match: PossibleView = {
      other: { entityId: 9, typeId: "person", name: "Dupond", status: "active" },
      reason: "phonetic",
      score: 0.8,
      state: "open",
    };

    const html = page("fr-FR", createElement(PossibleList, { matches: [match], onOpen: () => {} }));

    expect(html).toContain("Dupond");
    expect(html).toContain("à trancher");
    expect(html).toContain("ils se prononcent de la même façon");
  });

  it("draws how a spreadsheet's columns were read, read or not", () => {
    const columns: ColumnView[] = [
      { sheet: "Mars", column: "Medecin", semanticType: "person", subtype: null, role: "provider", confidence: 0.9, reason: "header_exact", read: true },
      { sheet: "Mars", column: "Duree", semanticType: "none", subtype: null, role: null, confidence: 0.1, reason: "no_header_match", read: false },
    ];

    const html = page(
      "fr-FR",
      createElement(ColumnList, { workbooks: [{ relativePath: "rdv.xlsx", columns }] }),
    );

    expect(html).toContain("Mars / Medecin");
    expect(html).toContain("lue comme Personne");
    expect(html).toContain("titre de colonne reconnu");
    expect(html).toContain("Mars / Duree");
    expect(html).toContain("non lue");
    expect(html).toContain("titre de colonne inconnu");
  });

  it("counts the mentions and the files under the title", () => {
    const html = page("fr-FR", createElement(SheetCounts, { detail: detail() }));

    expect(html).toContain("Personne · 2 mentions · 2 fichiers");
  });

  it("draws the link under an analysis summary in both languages", () => {
    expect(page("fr-FR", createElement(KnowledgeLink, { onOpen: () => {} }))).toContain(
      "Voir les connaissances",
    );
    expect(page("en-US", createElement(KnowledgeLink, { onOpen: () => {} }))).toContain(
      "See the knowledge",
    );
  });
});

describe("a source over its cap", () => {
  it("says that the names listed for it are not all of them", () => {
    const html = provenance("fr-FR", {
      sources: [
        { sourceId: 1, domain: "documents", relativePath: "livre.pdf", truncated: true, mentions: [mention({})] },
      ],
    });

    expect(html).toContain("plus de noms que le maximum retenu");
  });
});
