import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, lookupKey, type Catalogue } from "../i18n/translate";
import type { Suggestion, SuggestionSet, SuggestionTemplate } from "./ipc";
import {
  monthName,
  namesSomebody,
  suggestionKey,
  suggestionText,
  suggestionView,
} from "./suggestions";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function suggestion(overrides: Partial<Suggestion> & { templateId: SuggestionTemplate }): Suggestion {
  return {
    entity: null,
    value: null,
    column: null,
    group: null,
    file: null,
    month: null,
    year: null,
    ...overrides,
  };
}

const alice = { entityId: 1, display: "Alice Archer" };

const everyKind: Suggestion[] = [
  suggestion({ templateId: "entity_documents", entity: alice }),
  suggestion({ templateId: "entity_rows_count", entity: alice, value: "Alice Archer" }),
  suggestion({
    templateId: "entity_total",
    entity: alice,
    value: "Alice Archer",
    column: "montant",
  }),
  suggestion({
    templateId: "entity_period_count",
    entity: alice,
    value: "Alice Archer",
    month: 3,
    year: 2026,
  }),
  suggestion({ templateId: "entity_compare_docs_data", entity: alice }),
  suggestion({ templateId: "top_group", column: "montant", group: "medecin" }),
  suggestion({ templateId: "each_document" }),
  suggestion({ templateId: "folder_count" }),
];

describe("the sentence of a suggestion", () => {
  it("is written from the catalogue, in the language of the interface, with every slot filled", () => {
    for (const each of everyKind) {
      for (const [t, locale] of [
        [french, "fr-FR"],
        [english, "en-US"],
      ] as const) {
        const text = suggestionText(t, locale, each);
        expect(text, `${locale} ${each.templateId}`).not.toMatch(/[{}]/);
        expect(text.length).toBeGreaterThan(8);
      }
    }
  });

  it("says the row question about a name the way the tables read it", () => {
    const rows = suggestion({ templateId: "entity_rows_count", entity: alice, value: "Alice Archer" });
    const period = everyKind[3] as Suggestion;
    const total = everyKind[2] as Suggestion;

    expect(suggestionText(french, "fr-FR", rows)).toBe("Combien de lignes pour Alice Archer ?");
    expect(suggestionText(english, "en-US", rows)).toBe("How many rows for Alice Archer?");
    expect(suggestionText(french, "fr-FR", total)).toBe(
      "Quelle est la somme de montant pour Alice Archer ?",
    );
    expect(suggestionText(french, "fr-FR", period)).toBe(
      "Combien de lignes pour Alice Archer en mars 2026 ?",
    );
    expect(suggestionText(english, "en-US", period)).toBe(
      "How many rows for Alice Archer in March 2026?",
    );
  });

  it("names the workbook when the suggestion carries one, and only for the kinds that have a variant", () => {
    const named = suggestion({
      templateId: "entity_rows_count",
      entity: alice,
      value: "Alice Archer",
      file: "rdv.csv",
    });
    const group = suggestion({ templateId: "top_group", column: "montant", group: "medecin", file: "rdv.csv" });
    const documents = suggestion({ templateId: "entity_documents", entity: alice, file: "rdv.csv" });

    expect(suggestionKey(named)).toBe("suggestions.entity_rows_countInFile");
    expect(suggestionKey(group)).toBe("suggestions.top_groupInFile");
    expect(suggestionKey(documents)).toBe("suggestions.entity_documents");
    expect(suggestionText(french, "fr-FR", named)).toBe(
      "Combien de lignes pour Alice Archer dans rdv.csv ?",
    );
    expect(suggestionText(english, "en-US", group)).toBe(
      "What is the sum of montant by medecin in rdv.csv?",
    );
  });

  it("gives the month in the language of the interface", () => {
    expect(monthName("fr-FR", 3)).toBe("mars");
    expect(monthName("fr-FR", 8)).toBe("août");
    expect(monthName("en-US", 3)).toBe("March");
    expect(monthName("en-US", 12)).toBe("December");
  });
});

describe("which suggestions put a name on the screen", () => {
  it("is every kind that has an entity", () => {
    expect(everyKind.filter(namesSomebody).map((each) => each.templateId)).toEqual([
      "entity_documents",
      "entity_rows_count",
      "entity_total",
      "entity_period_count",
      "entity_compare_docs_data",
    ]);
  });
});

describe("the view of a set", () => {
  const hidden: SuggestionSet = {
    suggestions: [everyKind[5] as Suggestion, everyKind[6] as Suggestion],
    namesHidden: true,
    dryRuns: 1,
  };
  const shown: SuggestionSet = { suggestions: everyKind, namesHidden: false, dryRuns: 4 };

  it("offers to show the names while they are hidden and there are some to show", () => {
    const view = suggestionView(french, "fr-FR", hidden, false);

    expect(view.items).toHaveLength(2);
    expect(view.canRevealNames).toBe(true);
    expect(view.canHideNames).toBe(false);
    expect(view.items.map((item) => item.text).join(" ")).not.toContain("Alice");
  });

  it("offers to hide them again once the user showed them, and not before", () => {
    const view = suggestionView(french, "fr-FR", shown, true);

    expect(view.canRevealNames).toBe(false);
    expect(view.canHideNames).toBe(true);
    expect(suggestionView(french, "fr-FR", shown, false).canHideNames).toBe(false);
  });

  it("offers neither when there was nothing to hide and nothing to hide again", () => {
    const view = suggestionView(
      french,
      "fr-FR",
      { suggestions: [everyKind[6] as Suggestion], namesHidden: false, dryRuns: 0 },
      false,
    );

    expect(view.canRevealNames).toBe(false);
    expect(view.canHideNames).toBe(false);
  });

  it("draws nothing without a set", () => {
    expect(suggestionView(french, "fr-FR", null, false)).toEqual({
      items: [],
      canRevealNames: false,
      canHideNames: false,
    });
  });

  it("gives every item its own key", () => {
    const keys = suggestionView(french, "fr-FR", shown, true).items.map((item) => item.key);

    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe("the catalogue and the kinds Rust can send", () => {
  const here = fileURLToPath(new URL(".", import.meta.url));
  const rust = readFileSync(
    resolve(here, "../../src-tauri/src/knowledge/suggest.rs"),
    "utf8",
  );
  const ids = [...rust.matchAll(/pub const [A-Z_]+: &str = "([a-z_]+)";/g)].map((m) => m[1] as string);

  it("finds the template ids in the Rust source", () => {
    expect(ids).toEqual([
      "entity_documents",
      "entity_rows_count",
      "entity_total",
      "entity_period_count",
      "entity_compare_docs_data",
      "top_group",
      "each_document",
      "folder_count",
    ]);
  });

  it.each(["fr-FR", "en-US"])("%s has a sentence for every kind, and a variant that names the workbook", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;
    for (const id of ids) {
      expect(lookupKey(catalogue, `suggestions.${id}`), `${id}`).toBeTypeOf("string");
    }
    for (const id of ["entity_rows_count", "entity_total", "entity_period_count", "top_group"]) {
      const variant = lookupKey(catalogue, `suggestions.${id}InFile`);
      expect(variant, `${id}InFile`).toContain("{file}");
    }
  });

  it.each(["fr-FR", "en-US"])("%s sentences use only slots Rust fills", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;
    const known = new Set(["name", "value", "column", "group", "file", "month", "year"]);
    for (const id of ids) {
      for (const key of [id, `${id}InFile`]) {
        const text = lookupKey(catalogue, `suggestions.${key}`);
        if (typeof text !== "string") {
          continue;
        }
        for (const [, slot] of text.matchAll(/\{([a-z]+)\}/g)) {
          expect(known.has(slot as string), `${locale} ${key} uses {${slot}}`).toBe(true);
        }
      }
    }
  });
});
