import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { cardView } from "./entityCard";
import type { EntityCard } from "./ipc";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function card(overrides: Partial<EntityCard> = {}): EntityCard {
  return {
    entityId: 1,
    display: "Alice Archer",
    typeId: "person",
    subtype: null,
    status: "active",
    aliases: ["Archer Alice", "A. Archer"],
    roles: [{ role: "client", mentions: 2 }],
    documents: [{ relativePath: "archer-courrier.txt", mentions: 1, pages: [1] }],
    tables: [{ relativePath: "rdv.csv", sheet: "rdv", column: "patient", rows: 2 }],
    outside: { documents: 0, workbooks: 0 },
    relations: [
      { predicate: "served_by", otherEntityId: 6, otherDisplay: "Bruno Baker", direction: "subject" },
      { predicate: "served_by", otherEntityId: 8, otherDisplay: "Dora Durand", direction: "object" },
    ],
    attributes: [{ key: "spoken_form", value: "Alisse" }],
    ...overrides,
  };
}

describe("the sheet of a name, in words", () => {
  it("lists the documents with their pages and the tables with the sheet and the column", () => {
    const view = cardView(french, card());

    expect(view.title).toBe("Fiche de Alice Archer");
    expect(view.kind).toBe("Personne");
    expect(view.aliases).toEqual(["Archer Alice", "A. Archer"]);
    expect(view.documents).toEqual([
      { key: "archer-courrier.txt", text: "archer-courrier.txt : 1 mention, page 1" },
    ]);
    expect(view.tables[0]?.text).toBe("rdv.csv, feuille rdv, colonne patient : 2 lignes");
    expect(view.nowhere).toBe(false);
    expect(view.outside).toBeNull();
  });

  it("says the same in English", () => {
    const view = cardView(english, card({ documents: [{ relativePath: "a.txt", mentions: 3, pages: [2, 5] }] }));

    expect(view.title).toBe("Sheet of Alice Archer");
    expect(view.documents[0]?.text).toBe("a.txt: 3 mentions, pages 2, 5");
    expect(view.tables[0]?.text).toBe("rdv.csv, sheet rdv, column patient: 2 rows");
  });

  it("leaves the pages out for a file that has none, and counts a single row in the singular", () => {
    const view = cardView(
      french,
      card({
        documents: [{ relativePath: "note.txt", mentions: 1, pages: [] }],
        tables: [{ relativePath: "rdv.csv", sheet: "rdv", column: "patient", rows: 1 }],
      }),
    );

    expect(view.documents[0]?.text).toBe("note.txt : 1 mention");
    expect(view.tables[0]?.text).toContain("1 ligne");
    expect(view.tables[0]?.text).not.toContain("lignes");
  });

  it("only counts what lies outside the selection, and never names a file there", () => {
    const view = cardView(
      french,
      card({ documents: [], tables: [], outside: { documents: 2, workbooks: 1 } }),
    );

    expect(view.nowhere).toBe(true);
    expect(view.outside).toBe(
      "Hors de la sélection : 2 dans les documents, 1 dans les tableaux. La sélection n'est pas changée.",
    );
  });

  it("says nothing about the outside when there is nothing outside", () => {
    expect(cardView(french, card()).outside).toBeNull();
  });

  it("marks a name the base is not sure of", () => {
    expect(cardView(french, card({ status: "candidate" })).unconfirmed).toBe(true);
    expect(cardView(french, card()).unconfirmed).toBe(false);
  });

  it("writes a link from either end", () => {
    const view = cardView(french, card());

    expect(view.relations.map((line) => line.text)).toEqual([
      "Alice Archer suivi par Bruno Baker",
      "Dora Durand suivi par Alice Archer",
    ]);
  });

  it("writes a role through the label the pack gave it, and keeps an unknown id as it is", () => {
    const view = cardView(
      french,
      card({
        roles: [
          { role: "client", mentions: 2 },
          { role: "zzz_unknown", mentions: 1 },
        ],
      }),
    );

    expect(view.roles[1]).toBe("zzz_unknown (1 mention)");
    expect(view.roles[0]).toMatch(/\(2 mentions\)$/);
  });

  it("writes the kind with a subtype the build labels, and without one it does not know", () => {
    expect(cardView(french, card({ subtype: "no_such_subtype" })).kind).toBe("Personne");
  });

  it("writes the details as key and value", () => {
    expect(cardView(french, card()).details[0]?.text).toBe("spoken_form : Alisse");
    expect(cardView(english, card()).details[0]?.text).toBe("spoken_form: Alisse");
  });
});
