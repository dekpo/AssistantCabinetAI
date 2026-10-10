import { describe, expect, it } from "vitest";
import { createTranslator, type Catalogue } from "../i18n/translate";
import enUS from "../locales/en-US.json";
import frFR from "../locales/fr-FR.json";
import { knowledgeLines } from "./knowledgeSummary";
import type { KnowledgeSummary } from "./ipc";

const reference = enUS as Catalogue;
const french = createTranslator(frFR as Catalogue, reference);
const english = createTranslator(reference, reference);

function summary(overrides: Partial<KnowledgeSummary> = {}): KnowledgeSummary {
  return {
    entitiesDetected: 0,
    matchedExisting: 0,
    createdNew: 0,
    candidates: 0,
    identifiers: 0,
    relations: 0,
    refreshedSources: 0,
    truncatedSources: 0,
    errors: 0,
    elapsedMs: 0,
    ...overrides,
  };
}

describe("the knowledge lines of the analysis summary", () => {
  it("say nothing when the knowledge base took no part or found nothing", () => {
    expect(knowledgeLines(french, null)).toEqual([]);
    expect(knowledgeLines(french, undefined)).toEqual([]);
    expect(knowledgeLines(french, summary())).toEqual([]);
  });

  it("write the names found, and how many were already known, in French", () => {
    const lines = knowledgeLines(french, summary({ entitiesDetected: 12, matchedExisting: 9 }));
    expect(lines).toEqual(["12 personnes et organisations repérées, dont 9 déjà connues."]);
  });

  it("use the singular for one, and for none that were known", () => {
    expect(
      knowledgeLines(french, summary({ entitiesDetected: 1, matchedExisting: 0 })),
    ).toEqual(["1 personne ou organisation repérée, dont 0 déjà connue."]);
    expect(
      knowledgeLines(french, summary({ entitiesDetected: 2, matchedExisting: 1 })),
    ).toEqual(["2 personnes et organisations repérées, dont 1 déjà connue."]);
  });

  it("write the same facts in English", () => {
    const lines = knowledgeLines(english, summary({ entitiesDetected: 3, matchedExisting: 1 }));
    expect(lines).toEqual(["3 people and organisations found, of which 1 already known."]);
  });

  it("put the line that asks her to act first", () => {
    const lines = knowledgeLines(
      french,
      summary({ errors: 2, entitiesDetected: 4, identifiers: 1, refreshedSources: 3 }),
    );
    expect(lines[0]).toContain("2 documents n'ont pas pu être entièrement analysés");
    expect(lines[0]).toContain("relancez l'analyse");
    expect(lines).toHaveLength(4);
  });

  it("count identifiers apart from names, and say what was read again without the AI", () => {
    const lines = knowledgeLines(french, summary({ identifiers: 2, refreshedSources: 1 }));
    expect(lines).toEqual([
      "2 identifiants repérés (adresses e-mail, numéros de facture, IBAN...).",
      "1 document déjà analysé relu pour y repérer les noms, sans solliciter l'IA.",
    ]);
  });

  it("report documents that held more names than are kept", () => {
    const lines = knowledgeLines(french, summary({ truncatedSources: 1 }));
    expect(lines).toEqual([
      "1 document contient plus de noms que la base n'en retient : seuls les plus sûrs sont conservés.",
    ]);
  });

  it("never carries a name: it is given none", () => {
    // The summary type has no string field at all; this is the structural reason.
    const value = summary({ entitiesDetected: 1 });
    expect(Object.values(value).every((field) => typeof field === "number")).toBe(true);
  });
});
