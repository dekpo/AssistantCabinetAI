import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { summaryStateLine, summaryUnitLabel, type SummaryPlan } from "./summaries";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function plan(over: Partial<SummaryPlan>): SummaryPlan {
  return {
    total: 12,
    done: 0,
    remaining: 12,
    estimatedSeconds: 240,
    stale: false,
    readsEverything: true,
    complete: false,
    ...over,
  };
}

describe("summaryStateLine", () => {
  it("says what a run would cost before anything starts", () => {
    const line = summaryStateLine(french, plan({}));
    expect(line).toContain("12");
    expect(line).toContain("4m0s");
  });

  it("says how many parts are read when a full reading is not finished", () => {
    const line = summaryStateLine(french, plan({ done: 5, remaining: 7, estimatedSeconds: 140 }));
    expect(line).toContain("5");
    expect(line).toContain("12");
    expect(line).not.toContain("entier");
  });

  it("calls a full reading complete only when every part was read", () => {
    expect(summaryStateLine(french, plan({ done: 12, remaining: 0, complete: true }))).toContain("entier");
    expect(summaryStateLine(english, plan({ done: 12, remaining: 0, complete: true }))).toContain("whole");
  });

  it("never calls the chapters a full reading, even when all are written", () => {
    const line = summaryStateLine(
      french,
      plan({ done: 12, remaining: 0, complete: true, readsEverything: false }),
    );
    expect(line).not.toContain("entier");
    expect(line).toContain("chapitre");
  });

  it("warns that earlier summaries are out of date", () => {
    expect(summaryStateLine(french, plan({ stale: true }))).toContain("modifié");
  });
});

describe("summaryUnitLabel", () => {
  it("names the chapter, or the pages when the document has no title there", () => {
    expect(summaryUnitLabel(french, { title: "Franchise", firstPage: 41, lastPage: 44 })).toBe("Franchise (pages 41 à 44)");
    expect(summaryUnitLabel(french, { title: "", firstPage: 7, lastPage: 7 })).toBe("page 7");
    expect(summaryUnitLabel(english, { title: "", firstPage: 21, lastPage: 40 })).toBe("pages 21 to 40");
  });
});
