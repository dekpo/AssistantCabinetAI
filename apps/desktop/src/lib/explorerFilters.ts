import { ENTITY_TYPE, type EntityKind, type KnowledgeFacts } from "./explorerFacts";
import {
  entityChoice,
  filterEntries,
  folderKey,
  parseEntityChoice,
  type ExplorerEntry,
  type ExplorerFilter,
  type ExplorerRoot,
  type ExplorerState,
} from "./explorerModel";
import type { FinderReason } from "./ipc";

/*
 * What the filter panel of the explorer knows and decides, with no React: the number shown beside
 * each choice, the suggestions of the origin-folder field, the list of active pills and how each one
 * is removed, and the short reason shown on a row of a text search. The filter type itself and
 * `filterEntries` stay in `explorerModel.ts`; this module only reads them.
 *
 * Counting rule (owner brief, step 3): the number beside a choice is counted over what **all the
 * other filters** leave, with the filter of the choice's own kind left out. So ticking a choice
 * never leads to an empty list without the number having said so, and the choices of one kind
 * combine as "this or that" while the kinds combine as "this and that".
 *
 * A typed extension (`.docx`, `xlsx`) is read by the finder, not here (`finder/typed.rs`): the
 * answer echoes the extensions it took out of the text, and this module only lights the matching
 * choice from that echo. A typed extension **narrows** like any other filter: the search finds the
 * docx files, and a format choice then picks among them, so a typed `.docx` beside a `pdf` choice
 * shows nothing (the `pdf` choice says 0 before it is ticked). It never widens to "pdf or docx".
 */

/**
 * The kinds of choice the panel offers. The family is the segmented control, outside the panel. `recent`
 * (the newest files first) and `conversation` (only the files the conversation uses) are single
 * switches that the built-in views set; `conversation` narrows, `recent` only orders.
 */
export type FilterKind =
  | "format"
  | "state"
  | "folder"
  | "person"
  | "organisation"
  | "type"
  | "year"
  | "recent"
  | "conversation";

/** The most suggestions the origin-folder field offers; the rest is counted, not listed. */
export const MAX_FOLDER_SUGGESTIONS = 5;

/** The filter with the choices of one kind left out: what that kind's counts are worked over. */
function without(filter: ExplorerFilter, kind: FilterKind | "family"): ExplorerFilter {
  return {
    ...filter,
    family: kind === "family" ? null : filter.family,
    formats: kind === "format" ? undefined : filter.formats,
    states: kind === "state" ? undefined : filter.states,
    folders: kind === "folder" ? undefined : filter.folders,
    people: kind === "person" ? undefined : filter.people,
    organisations: kind === "organisation" ? undefined : filter.organisations,
    types: kind === "type" ? undefined : filter.types,
    years: kind === "year" ? undefined : filter.years,
    ticked: kind === "conversation" ? null : filter.ticked,
  };
}

/** The entries the other filters leave, for counting the choices of `kind`. */
export function leftByOthers(
  entries: readonly ExplorerEntry[],
  filter: ExplorerFilter,
  kind: FilterKind | "family",
): ExplorerEntry[] {
  return filterEntries(entries, without(filter, kind));
}

/** One format choice: an extension present in the folders, with its count. */
export interface FormatChoice {
  extension: string;
  count: number;
}

/** The formats present among the entries, most files first, then by name. A format ticked or typed
 * is always listed, even if no file is left for it. */
export function formatChoices(
  entries: readonly ExplorerEntry[],
  filter: ExplorerFilter,
  typedFormats: readonly string[],
): FormatChoice[] {
  const left = leftByOthers(entries, filter, "format");
  const counts = new Map<string, number>();
  for (const entry of entries) {
    counts.set(entry.extension, counts.get(entry.extension) ?? 0);
  }
  for (const entry of left) {
    counts.set(entry.extension, (counts.get(entry.extension) ?? 0) + 1);
  }
  for (const extension of [...(filter.formats ?? []), ...typedFormats]) {
    counts.set(extension, counts.get(extension) ?? 0);
  }
  return [...counts]
    .filter(([extension]) => extension !== "")
    .map(([extension, count]) => ({ extension, count }))
    .sort((a, b) => b.count - a.count || a.extension.localeCompare(b.extension));
}

/** The states of the screen, in the order the panel draws them. */
export const STATE_CHOICES: readonly ExplorerState[] = ["analysed", "toAnalyse", "unreadable"];

/** How many files each state holds among what the other filters leave. */
export function stateCounts(
  entries: readonly ExplorerEntry[],
  filter: ExplorerFilter,
): Record<ExplorerState, number> {
  const counts: Record<ExplorerState, number> = { analysed: 0, toAnalyse: 0, unreadable: 0 };
  for (const entry of leftByOthers(entries, filter, "state")) {
    counts[entry.state] += 1;
  }
  return counts;
}

/** One origin folder the field can offer. */
export interface FolderChoice {
  key: string;
  root: ExplorerRoot;
  /** The folder inside its root, with `/` as separator. */
  folder: string;
  /** The files in it or under it, among what the other filters leave. */
  count: number;
  /** The two roots hold a folder of this name: the choice says which one it is. */
  ambiguous: boolean;
}

/** The folder of a key built by `folderKey`. */
export function folderOfKey(key: string): string {
  return key.slice(key.indexOf(":") + 1);
}

/** The root of a key built by `folderKey`. */
export function rootOfKey(key: string): ExplorerRoot {
  return key.startsWith("data:") ? "data" : "documents";
}

/** The folder with its ancestors: `a/b/c` is `a`, `a/b`, `a/b/c`. */
function withAncestors(folder: string): string[] {
  const parts = folder.split("/");
  return parts.map((_, index) => parts.slice(0, index + 1).join("/"));
}

/** Lower case, no accents: how the field compares what was typed with a folder name. */
function plain(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase();
}

/** The origin folders of the entries (every folder and each of its ancestors), each counted over the
 * files in it or under it that the other filters leave. A file at the top of its root has no folder
 * and is in none. Not sorted. */
export function folderChoices(
  entries: readonly ExplorerEntry[],
  filter: ExplorerFilter,
): FolderChoice[] {
  const counts = new Map<string, { root: ExplorerRoot; folder: string; count: number }>();
  for (const entry of entries) {
    if (entry.folder === "") {
      continue;
    }
    for (const folder of withAncestors(entry.folder)) {
      const key = folderKey(entry.root, folder);
      if (!counts.has(key)) {
        counts.set(key, { root: entry.root, folder, count: 0 });
      }
    }
  }
  for (const entry of leftByOthers(entries, filter, "folder")) {
    if (entry.folder === "") {
      continue;
    }
    for (const folder of withAncestors(entry.folder)) {
      const found = counts.get(folderKey(entry.root, folder));
      if (found !== undefined) {
        found.count += 1;
      }
    }
  }
  const roots = new Map<string, Set<ExplorerRoot>>();
  for (const { root, folder } of counts.values()) {
    const key = plain(folder);
    roots.set(key, (roots.get(key) ?? new Set()).add(root));
  }
  return [...counts].map(([key, { root, folder, count }]) => ({
    key,
    root,
    folder,
    count,
    ambiguous: (roots.get(plain(folder))?.size ?? 0) > 1,
  }));
}

/** What the origin-folder field offers for what was typed. */
export interface FolderSuggestions {
  /** At most `MAX_FOLDER_SUGGESTIONS`: the ones with the most files, then by name. */
  shown: FolderChoice[];
  /** How many more match: told as "N autres : tapez pour affiner", never listed. */
  more: number;
}

/** The suggestions for `typed`: the folders whose name contains it (accents and case ignored), minus
 * the ones already chosen. */
export function suggestFolders(
  choices: readonly FolderChoice[],
  typed: string,
  chosen: ReadonlySet<string>,
  limit: number = MAX_FOLDER_SUGGESTIONS,
): FolderSuggestions {
  const wanted = plain(typed.trim());
  const matching = choices
    .filter((choice) => !chosen.has(choice.key) && plain(choice.folder).includes(wanted))
    .sort((a, b) => b.count - a.count || a.folder.localeCompare(b.folder) || a.root.localeCompare(b.root));
  return { shown: matching.slice(0, limit), more: Math.max(0, matching.length - limit) };
}

/** One business type choice: a code the profile gave some files, with its count. */
export interface TypeChoice {
  code: string;
  count: number;
}

/** The business types of the analysed files, most files first, then by code. A type chosen is always
 * listed, even if no file is left for it. */
export function typeChoices(entries: readonly ExplorerEntry[], filter: ExplorerFilter): TypeChoice[] {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    if (entry.facts?.type != null) {
      counts.set(entry.facts.type, counts.get(entry.facts.type) ?? 0);
    }
  }
  for (const entry of leftByOthers(entries, filter, "type")) {
    if (entry.facts?.type != null) {
      counts.set(entry.facts.type, (counts.get(entry.facts.type) ?? 0) + 1);
    }
  }
  for (const code of filter.types ?? []) {
    counts.set(code, counts.get(code) ?? 0);
  }
  return [...counts]
    .map(([code, count]) => ({ code, count }))
    .sort((a, b) => b.count - a.count || a.code.localeCompare(b.code));
}

/** One year choice. */
export interface YearChoice {
  year: string;
  count: number;
}

/** The years of the analysed files, newest first. A file spanning several years counts in each. A year
 * chosen is always listed. */
export function yearChoices(entries: readonly ExplorerEntry[], filter: ExplorerFilter): YearChoice[] {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    for (const year of entry.facts?.years ?? []) {
      counts.set(year, counts.get(year) ?? 0);
    }
  }
  for (const entry of leftByOthers(entries, filter, "year")) {
    for (const year of entry.facts?.years ?? []) {
      counts.set(year, (counts.get(year) ?? 0) + 1);
    }
  }
  for (const year of filter.years ?? []) {
    counts.set(year, counts.get(year) ?? 0);
  }
  return [...counts]
    .map(([year, count]) => ({ year, count }))
    .sort((a, b) => b.year.localeCompare(a.year));
}

/** The most suggestions a typed field offers; the rest is counted, not listed. */
export const MAX_ENTITY_SUGGESTIONS = 5;

/** The roles worth offering as a second step: a name met in the text with no role is not one. */
export const ROLE_MENTION = "mention";

/** One person or organisation the field can offer. */
export interface EntityOption {
  entityId: number;
  /** The name the base gives it, or null when the base no longer knows it (a view that names it says so). */
  name: string | null;
  /** The files linked to it among what the other filters leave. */
  count: number;
  /** The roles that link it to those files, with their counts; the second step lists them. */
  roles: { role: string; count: number }[];
}

/** The people or the organisations the files link to, each counted over the files the other filters
 * leave (the kind's own choices left out). One that is chosen is always listed, known or not. Not
 * sorted. */
export function entityOptions(
  entries: readonly ExplorerEntry[],
  filter: ExplorerFilter,
  kind: EntityKind,
  facts: KnowledgeFacts,
): EntityOption[] {
  const options = new Map<number, { count: number; roles: Map<string, number> }>();
  const wanted = ENTITY_TYPE[kind];
  const optionOf = (entityId: number) => {
    let option = options.get(entityId);
    if (option === undefined) {
      option = { count: 0, roles: new Map() };
      options.set(entityId, option);
    }
    return option;
  };
  for (const entry of entries) {
    for (const link of entry.facts?.links ?? []) {
      if (facts.entities.get(link.entityId)?.typeId === wanted) {
        optionOf(link.entityId);
      }
    }
  }
  for (const entry of leftByOthers(entries, filter, kind)) {
    for (const link of entry.facts?.links ?? []) {
      if (facts.entities.get(link.entityId)?.typeId !== wanted) {
        continue;
      }
      const option = optionOf(link.entityId);
      option.count += 1;
      for (const role of link.roles) {
        option.roles.set(role, (option.roles.get(role) ?? 0) + 1);
      }
    }
  }
  for (const value of (kind === "person" ? filter.people : filter.organisations) ?? []) {
    optionOf(parseEntityChoice(value).entityId);
  }
  return [...options].map(([entityId, { count, roles }]) => ({
    entityId,
    name: facts.entities.get(entityId)?.name ?? null,
    count,
    roles: [...roles]
      .map(([role, roleCount]) => ({ role, count: roleCount }))
      .sort((a, b) => b.count - a.count || a.role.localeCompare(b.role)),
  }));
}

/** What a typed field of people or organisations offers for what was typed. */
export interface EntitySuggestions {
  /** At most `MAX_ENTITY_SUGGESTIONS`: the ones with the most files, then by name. */
  shown: EntityOption[];
  /** How many more match: told as "N autres : tapez pour affiner", never listed. */
  more: number;
}

/** The suggestions for `typed`: the names that contain it (accents and case ignored), minus the ones
 * already chosen and the ones the base no longer knows. Nothing typed offers the most frequent. */
export function suggestEntities(
  options: readonly EntityOption[],
  typed: string,
  chosen: ReadonlySet<number>,
  limit: number = MAX_ENTITY_SUGGESTIONS,
): EntitySuggestions {
  const wanted = plain(typed.trim());
  const matching = options
    .filter(
      (option) =>
        option.name !== null &&
        !chosen.has(option.entityId) &&
        plain(option.name).includes(wanted),
    )
    .sort(
      (a, b) =>
        b.count - a.count ||
        (a.name ?? "").localeCompare(b.name ?? "") ||
        a.entityId - b.entityId,
    );
  return { shown: matching.slice(0, limit), more: Math.max(0, matching.length - limit) };
}

/** The ids of the people or organisations among the choices, whatever role they were chosen for. */
export function chosenEntityIds(chosen: ReadonlySet<string>): Set<number> {
  return new Set([...chosen].map((value) => parseEntityChoice(value).entityId));
}

/** One removable pill under the segmented control. `typed` pills come from the search text: removing
 * one takes the extension out of the text (`withoutTypedFormat`), the others empty a set. */
export interface ActivePill {
  kind: FilterKind;
  /** The extension, the state, or the folder key. */
  value: string;
  typed: boolean;
}

/** The two switches the built-in views set, outside the sets of the filter. */
export interface Switches {
  recent: boolean;
  conversation: boolean;
}

const NO_SWITCHES: Switches = { recent: false, conversation: false };

/** The active pills in a stable order: formats (typed first), states, folders, people, organisations,
 * types, years, then the two switches. */
export function activePills(
  filter: ExplorerFilter,
  typedFormats: readonly string[],
  switches: Switches = NO_SWITCHES,
): ActivePill[] {
  const pills: ActivePill[] = typedFormats.map((value) => ({ kind: "format", value, typed: true }));
  for (const value of filter.formats ?? []) {
    if (!typedFormats.includes(value)) {
      pills.push({ kind: "format", value, typed: false });
    }
  }
  for (const state of STATE_CHOICES) {
    if (filter.states?.has(state)) {
      pills.push({ kind: "state", value: state, typed: false });
    }
  }
  for (const value of filter.folders ?? []) {
    pills.push({ kind: "folder", value, typed: false });
  }
  for (const value of filter.people ?? []) {
    pills.push({ kind: "person", value, typed: false });
  }
  for (const value of filter.organisations ?? []) {
    pills.push({ kind: "organisation", value, typed: false });
  }
  for (const value of filter.types ?? []) {
    pills.push({ kind: "type", value, typed: false });
  }
  for (const value of [...(filter.years ?? [])].sort()) {
    pills.push({ kind: "year", value, typed: false });
  }
  if (switches.conversation) {
    pills.push({ kind: "conversation", value: "conversation", typed: false });
  }
  if (switches.recent) {
    pills.push({ kind: "recent", value: "recent", typed: false });
  }
  return pills;
}

/** The number on the « Filtres » button: the choices in force. */
export function activeFilterCount(
  filter: ExplorerFilter,
  typedFormats: readonly string[],
  switches?: Switches,
): number {
  return activePills(filter, typedFormats, switches).length;
}

/** The set of one kind without one value; the same set when it holds none. */
function minus<T>(set: ReadonlySet<T> | undefined, value: T): Set<T> {
  const next = new Set(set ?? []);
  next.delete(value);
  return next;
}

/** The set of one kind with one value added, or taken out when it is already there. */
function toggled<T>(set: ReadonlySet<T> | undefined, value: T): Set<T> {
  const next = new Set(set ?? []);
  if (!next.delete(value)) {
    next.add(value);
  }
  return next;
}

/** The panel's choices, as the box holds them. The family is held apart. */
export interface PanelChoices {
  formats: ReadonlySet<string>;
  states: ReadonlySet<ExplorerState>;
  folders: ReadonlySet<string>;
  /** `entityId` (any role) or `entityId:role`. */
  people: ReadonlySet<string>;
  organisations: ReadonlySet<string>;
  /** Business type codes. */
  types: ReadonlySet<string>;
  /** `YYYY`. */
  years: ReadonlySet<string>;
  /** The newest files first. Orders the lists, never narrows them. */
  recent: boolean;
  /** Only the files the conversation uses now. */
  inConversation: boolean;
}

export const NO_CHOICES: PanelChoices = {
  formats: new Set(),
  states: new Set(),
  folders: new Set(),
  people: new Set(),
  organisations: new Set(),
  types: new Set(),
  years: new Set(),
  recent: false,
  inConversation: false,
};

/** The choices after one was ticked, or unticked if it was ticked. A format is a string, a state is
 * one of the three, a folder is a key, a person or an organisation is an entity choice, a type is a
 * code, a year is `YYYY`. The two switches ignore the value. */
export function toggleChoice(choices: PanelChoices, kind: FilterKind, value: string): PanelChoices {
  switch (kind) {
    case "format":
      return { ...choices, formats: toggled(choices.formats, value) };
    case "state":
      return { ...choices, states: toggled(choices.states, value as ExplorerState) };
    case "folder":
      return { ...choices, folders: toggled(choices.folders, value) };
    case "person":
      return { ...choices, people: toggled(choices.people, value) };
    case "organisation":
      return { ...choices, organisations: toggled(choices.organisations, value) };
    case "type":
      return { ...choices, types: toggled(choices.types, value) };
    case "year":
      return { ...choices, years: toggled(choices.years, value) };
    case "recent":
      return { ...choices, recent: !choices.recent };
    case "conversation":
      return { ...choices, inConversation: !choices.inConversation };
  }
}

/** The choices after a pill was removed. A typed pill leaves the sets alone: the text changes. */
export function removePill(choices: PanelChoices, pill: ActivePill): PanelChoices {
  if (pill.typed) {
    return choices;
  }
  switch (pill.kind) {
    case "format":
      return { ...choices, formats: minus(choices.formats, pill.value) };
    case "state":
      return { ...choices, states: minus(choices.states, pill.value as ExplorerState) };
    case "folder":
      return { ...choices, folders: minus(choices.folders, pill.value) };
    case "person":
      return { ...choices, people: minus(choices.people, pill.value) };
    case "organisation":
      return { ...choices, organisations: minus(choices.organisations, pill.value) };
    case "type":
      return { ...choices, types: minus(choices.types, pill.value) };
    case "year":
      return { ...choices, years: minus(choices.years, pill.value) };
    case "recent":
      return { ...choices, recent: false };
    case "conversation":
      return { ...choices, inConversation: false };
  }
}

/** The role step: the person or organisation is kept for one role, or for any role (null). It is
 * chosen once, whatever it was chosen for before. */
export function setEntityRole(
  choices: PanelChoices,
  kind: EntityKind,
  entityId: number,
  role: string | null,
): PanelChoices {
  const own = kind === "person" ? choices.people : choices.organisations;
  const kept = [...own].filter((value) => parseEntityChoice(value).entityId !== entityId);
  const next = new Set([...kept, entityChoice(entityId, role)]);
  return kind === "person" ? { ...choices, people: next } : { ...choices, organisations: next };
}

/** The typed text without the words that are this extension (`.docx`, `DOCX`). The finder decided
 * that they were extensions; this only takes those words back out. */
export function withoutTypedFormat(text: string, extension: string): string {
  return text
    .split(/\s+/)
    .filter((word) => word !== "" && word.replace(/^\./, "").toLowerCase() !== extension)
    .join(" ");
}

/** The typed text without any of these extensions. */
export function withoutTypedFormats(text: string, extensions: readonly string[]): string {
  return extensions.reduce(withoutTypedFormat, text);
}

/** The reasons of each hit of a text search, by `explorerKey(root, path)`. */
export type ReasonsByKey = ReadonlyMap<string, readonly FinderReason[]>;

/**
 * The catalogue key of the short label a reason gets on a row, or null for a reason that says
 * nothing a person needs (an unknown detail). Words are told by where they met the file.
 */
export function reasonKey(reason: FinderReason): string | null {
  switch (reason.kind) {
    case "word":
      return reason.detail === null ? null : WORD_REASONS[reason.detail] ?? null;
    case "identifier":
      return "explorer.reason.identifier";
    case "date":
      return reason.detail === "name" ? "explorer.reason.dateName" : "explorer.reason.date";
    case "entity":
      return reason.detail === "mention"
        ? "explorer.reason.mention"
        : "explorer.reason.entity";
  }
}

const WORD_REASONS: Readonly<Record<string, string>> = {
  name: "explorer.reason.name",
  title: "explorer.reason.title",
  folder: "explorer.reason.folder",
  author: "explorer.reason.author",
  subject: "explorer.reason.subject",
  entities: "explorer.reason.entities",
  headers: "explorer.reason.headers",
  cells: "explorer.reason.cells",
  identifiers: "explorer.reason.identifier",
  profile: "explorer.reason.profile",
  passage: "explorer.reason.passage",
};

/** The order a file's reasons are told in: where a person looks first. */
const REASON_ORDER: readonly string[] = [
  "explorer.reason.name",
  "explorer.reason.title",
  "explorer.reason.folder",
  "explorer.reason.passage",
  "explorer.reason.cells",
  "explorer.reason.headers",
  "explorer.reason.identifier",
  "explorer.reason.entities",
  "explorer.reason.mention",
  "explorer.reason.entity",
  "explorer.reason.dateName",
  "explorer.reason.date",
  "explorer.reason.subject",
  "explorer.reason.author",
  "explorer.reason.profile",
];

/** The label keys of a file's reasons, each once, in reading order. */
export function reasonKeys(reasons: readonly FinderReason[]): string[] {
  const keys = new Set<string>();
  for (const reason of reasons) {
    const key = reasonKey(reason);
    if (key !== null) {
      keys.add(key);
    }
  }
  return [...keys].sort((a, b) => REASON_ORDER.indexOf(a) - REASON_ORDER.indexOf(b));
}
