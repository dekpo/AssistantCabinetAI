import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import {
  NO_FILTERS,
  applyFilters,
  facetChips,
  hasFilters,
  toggleFilter,
  vectorShareLabel,
} from "./finderFilters";
import type { FinderFacets } from "./ipc";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

const facets: FinderFacets = {
  byType: [
    { value: "invoice", count: 4, label: null },
    { value: "user_lease", count: 2, label: null },
  ],
  byYear: [
    { value: "2026", count: 5, label: null },
    { value: "2025", count: 1, label: null },
  ],
  byEntity: [],
  byFolder: [],
  byFormat: [{ value: "pdf", count: 6, label: null }],
  byState: [
    { value: "analysed", count: 5, label: null },
    { value: "unanalysed", count: 1, label: null },
  ],
};

describe("toggleFilter", () => {
  it("adds a value, removes it when chosen again, and keeps one year at a time", () => {
    let filters = toggleFilter(NO_FILTERS, "type", "invoice");
    expect(filters.kinds).toEqual(["invoice"]);
    filters = toggleFilter(filters, "type", "quote");
    expect(filters.kinds).toEqual(["invoice", "quote"]);
    filters = toggleFilter(filters, "type", "invoice");
    expect(filters.kinds).toEqual(["quote"]);
    filters = toggleFilter(filters, "year", "2026");
    filters = toggleFilter(filters, "year", "2025");
    expect(filters.year).toBe("2025");
    filters = toggleFilter(filters, "year", "2025");
    expect(filters.year).toBeNull();
  });

  it("knows when nothing is filtered", () => {
    expect(hasFilters(NO_FILTERS)).toBe(false);
    expect(hasFilters(toggleFilter(NO_FILTERS, "format", "pdf"))).toBe(true);
  });
});

describe("applyFilters", () => {
  it("turns the chosen chips into the finder's query fields", () => {
    let filters = toggleFilter(NO_FILTERS, "type", "invoice");
    filters = toggleFilter(filters, "year", "2026");
    filters = toggleFilter(filters, "format", "pdf");
    filters = toggleFilter(filters, "state", "unanalysed");
    expect(applyFilters({ text: "franchise" }, filters)).toEqual({
      text: "franchise",
      kinds: ["invoice"],
      date: { from: "2026", to: "2026" },
      formats: ["pdf"],
      states: ["unanalysed"],
    });
  });

  it("leaves a query alone when nothing is chosen", () => {
    expect(applyFilters({ text: "x" }, NO_FILTERS)).toEqual({ text: "x" });
  });
});

describe("facetChips", () => {
  it("names a shipped type by the catalogue and a user type by its own label", () => {
    const chips = facetChips(french, facets, NO_FILTERS, { user_lease: "Bail" });
    const types = chips.filter((chip) => chip.group === "type");
    expect(types.map((chip) => chip.label)).toEqual(["Facture", "Bail"]);
    expect(types.map((chip) => chip.count)).toEqual([4, 2]);
  });

  it("writes the state of a file in the user's language", () => {
    const states = facetChips(french, facets, NO_FILTERS, {}).filter((chip) => chip.group === "state");
    expect(states.map((chip) => chip.label)).toEqual(["Analysé", "Non analysé"]);
    const english_ = facetChips(english, facets, NO_FILTERS, {}).filter((chip) => chip.group === "state");
    expect(english_.map((chip) => chip.label)).toEqual(["Analysed", "Not analysed"]);
  });

  it("marks the chips that are chosen", () => {
    const filters = toggleFilter(NO_FILTERS, "format", "pdf");
    const chips = facetChips(english, facets, filters, {});
    expect(chips.find((chip) => chip.group === "format")?.active).toBe(true);
    expect(chips.find((chip) => chip.group === "year")?.active).toBe(false);
    expect(chips.find((chip) => chip.group === "format")?.label).toBe("PDF");
  });
});

describe("vectorShareLabel", () => {
  it("says the share of a file's passages that the search by meaning sees", () => {
    expect(vectorShareLabel(french, { chunks: 8, vectors: 5 })).toContain("62");
    expect(vectorShareLabel(english, { chunks: 8, vectors: 0 })).toContain("0%");
  });
});
