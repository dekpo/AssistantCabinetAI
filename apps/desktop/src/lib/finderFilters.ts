import type { Translator } from "../i18n/translate";
import type { FinderFacets, FinderFileState, FinderQuery } from "./ipc";

/** The chips the user chose over the facets the finder returns. A year is one at a time. */
export interface FinderFilters {
  kinds: string[];
  year: string | null;
  formats: string[];
  states: FinderFileState[];
}

export const NO_FILTERS: FinderFilters = { kinds: [], year: null, formats: [], states: [] };

export type FacetGroup = "type" | "year" | "format" | "state";

export function hasFilters(filters: FinderFilters): boolean {
  return (
    filters.kinds.length > 0 ||
    filters.year !== null ||
    filters.formats.length > 0 ||
    filters.states.length > 0
  );
}

function flip<T>(values: T[], value: T): T[] {
  return values.includes(value) ? values.filter((one) => one !== value) : [...values, value];
}

/** Choose a chip, or choose it off. */
export function toggleFilter(filters: FinderFilters, group: FacetGroup, value: string): FinderFilters {
  switch (group) {
    case "type":
      return { ...filters, kinds: flip(filters.kinds, value) };
    case "year":
      return { ...filters, year: filters.year === value ? null : value };
    case "format":
      return { ...filters, formats: flip(filters.formats, value) };
    case "state":
      return { ...filters, states: flip(filters.states, value as FinderFileState) };
  }
}

/** The finder's query with the chosen chips in its own fields; a query with no chip is returned as it is. */
export function applyFilters(query: FinderQuery, filters: FinderFilters): FinderQuery {
  const filtered: FinderQuery = { ...query };
  if (filters.kinds.length > 0) {
    filtered.kinds = filters.kinds;
  }
  if (filters.year !== null) {
    filtered.date = { from: filters.year, to: filters.year };
  }
  if (filters.formats.length > 0) {
    filtered.formats = filters.formats;
  }
  if (filters.states.length > 0) {
    filtered.states = filters.states;
  }
  return filtered;
}

export interface FacetChip {
  group: FacetGroup;
  value: string;
  label: string;
  count: number;
  active: boolean;
}

/** The most chips shown for one group: the rest is a longer list than the field has room for. */
const CHIPS_PER_GROUP = 6;

function typeLabel(t: Translator, code: string, userLabels: Record<string, string>): string {
  const own = userLabels[code];
  if (own !== undefined) {
    return own;
  }
  const shipped = t(`businessType.${code}`);
  // A missing key comes back as the key itself: say the code rather than a path of the catalogue.
  return shipped === `businessType.${code}` ? code : shipped;
}

/** The chips over the facets of a result, each named in the user's language. */
export function facetChips(
  t: Translator,
  facets: FinderFacets,
  filters: FinderFilters,
  userTypeLabels: Record<string, string>,
): FacetChip[] {
  const groups: [FacetGroup, FinderFacets["byType"], (value: string) => string, (value: string) => boolean][] = [
    [
      "type",
      facets.byType,
      (value) => typeLabel(t, value, userTypeLabels),
      (value) => filters.kinds.includes(value),
    ],
    ["year", facets.byYear, (value) => value, (value) => filters.year === value],
    ["format", facets.byFormat, (value) => value.toUpperCase(), (value) => filters.formats.includes(value)],
    [
      "state",
      facets.byState,
      (value) => t(`finder.state.${value}`),
      (value) => filters.states.includes(value as FinderFileState),
    ],
  ];
  const chips: FacetChip[] = [];
  for (const [group, facet, label, active] of groups) {
    for (const entry of facet.slice(0, CHIPS_PER_GROUP)) {
      chips.push({
        group,
        value: entry.value,
        label: label(entry.value),
        count: entry.count,
        active: active(entry.value),
      });
    }
  }
  return chips;
}

/** What the search by meaning sees of one file: the share of its passages that have a vector. */
export function vectorShareLabel(t: Translator, share: { chunks: number; vectors: number }): string {
  const percent = share.chunks === 0 ? 0 : Math.floor((share.vectors * 100) / share.chunks);
  return t("workFolder.meaningShare", { percent: String(percent) });
}
