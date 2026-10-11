import type { Translator } from "../i18n/translate";
import type {
  EntityListQuery,
  EntityListView,
  EntityRow,
  KnowledgeOverview,
  LastOperation,
  LocatorView,
  ManageOutcome,
} from "./ipc";
import { counted } from "./plural";

/**
 * What the dialog hands to each tab. `run` is the one door every write goes through: it shows the
 * busy state, reports a failure on the dialog (a busy database gets a retry), and on success reads
 * everything again so no tab keeps showing what the write changed. It resolves to the outcome, or to
 * `null` when the write failed, so a form can stay open on a failure.
 */
export interface ManageApi {
  run(operation: () => Promise<ManageOutcome>): Promise<ManageOutcome | null>;
  /** Raised after every successful write; a tab that reads the base reads it again when it moves. */
  revision: number;
  busy: boolean;
  /** Show this entity's sheet in the Entities tab. */
  openEntity(entityId: number): void;
}

/*
 * What the management dialog decides, as plain functions: which query a set of filters makes, how a
 * locator reads, which action asks for a confirmation and with which words, which entity a merge
 * keeps. The components only draw; everything that could be wrong is here, where a test can reach
 * it. Rust sends machine codes and ids; every word below comes from the catalogue.
 */

/** The filters of the Entities tab, as the controls hold them. Empty string means "no filter". */
export interface ListFilters {
  text: string;
  typeId: string;
  origin: "" | "automatic" | "manual";
  view: EntityListView;
  problemsOnly: boolean;
}

export const EMPTY_FILTERS: ListFilters = {
  text: "",
  typeId: "",
  origin: "",
  view: "live",
  problemsOnly: false,
};

/** The query Rust is asked for. The text is sent as typed; Rust folds it. */
export function listQueryFor(filters: ListFilters, page: number): EntityListQuery {
  return {
    text: filters.text.trim(),
    typeId: filters.typeId === "" ? null : filters.typeId,
    origin: filters.origin === "" ? null : filters.origin,
    view: filters.view,
    problemsOnly: filters.problemsOnly,
    page: Math.max(0, Math.floor(page)),
  };
}

/** A change of filter goes back to the first page: page 4 of the old list means nothing in the new. */
export function withFilter(filters: ListFilters, change: Partial<ListFilters>): ListFilters {
  return { ...filters, ...change };
}

/** At least one page, even when the list is empty. */
export function pageCount(total: number, pageSize: number): number {
  if (pageSize <= 0) {
    return 1;
  }
  return Math.max(1, Math.ceil(total / pageSize));
}

/** The page to show after the list changed under it (a deletion on the last page). */
export function clampPage(page: number, total: number, pageSize: number): number {
  return Math.min(Math.max(0, page), pageCount(total, pageSize) - 1);
}

export function entityTitle(entity: { name: string; disambiguator: string }): string {
  return entity.disambiguator === "" ? entity.name : `${entity.name} (${entity.disambiguator})`;
}

/** A label from the catalogue, or the one `other` key when the code is not one the build knows. */
function labelled(t: Translator, prefix: string, code: string, fallback: string): string {
  const key = `${prefix}.${code}`;
  const text = t(key);
  return text === key ? t(`${prefix}.${fallback}`) : text;
}

export function typeLabel(t: Translator, typeId: string): string {
  const key = `manage.type.${typeId}`;
  const text = t(key);
  return text === key ? "" : text;
}

/** The subtype in the user's language, or nothing for a subtype this build does not label. */
export function subtypeLabel(t: Translator, subtype: string | null): string | null {
  if (subtype === null) {
    return null;
  }
  const key = `manage.subtype.${subtype}`;
  const text = t(key);
  return text === key ? null : text;
}

export function statusLabel(t: Translator, status: string): string {
  return t(`manage.status.${status}`);
}

export function originLabel(t: Translator, origin: string): string {
  return t(`manage.origin.${origin}`);
}

export function aliasKindLabel(t: Translator, kind: string): string {
  return labelled(t, "manage.aliasKind", kind, "manual");
}

export function methodLabel(t: Translator, method: string): string {
  return labelled(t, "manage.method", method, "other");
}

export function reasonLabel(t: Translator, reason: string): string {
  return labelled(t, "manage.reason", reason, "other");
}

export function columnReasonLabel(t: Translator, reason: string): string {
  return labelled(t, "manage.columnReason", reason, "other");
}

export function predicateLabel(t: Translator, predicate: string): string {
  return labelled(t, "manage.predicate", predicate, "other");
}

/** A role, through the label key the pack gave it, or nothing when there is none. */
export function roleLabel(t: Translator, role: string | null): string | null {
  if (role === null) {
    return null;
  }
  const key = `knowledge.roles.${role}`;
  const text = t(key);
  return text === key ? null : text;
}

export function confidenceText(value: number): string {
  return `${Math.round(Math.min(1, Math.max(0, value)) * 100)} %`;
}

/** Where a mention was found, in words: "page 3, passage 2" or "sheet "March", column "Doctor", rows 2, 5, 9". */
export function locatorText(t: Translator, locator: LocatorView): string {
  switch (locator.kind) {
    case "chunk": {
      if (locator.page !== null && locator.section !== null) {
        return t("manage.locator.page", { page: locator.page, section: locator.section });
      }
      if (locator.page !== null) {
        return t("manage.locator.pageOnly", { page: locator.page });
      }
      return t("manage.locator.unknown");
    }
    case "cells": {
      const sheet = locator.sheet ?? "";
      const column = locator.column ?? "";
      const rows =
        locator.sampleRows.length > 0
          ? locator.sampleRows
          : locator.firstRow === null
            ? []
            : [locator.firstRow];
      return rows.length === 0
        ? t("manage.locator.cellsNoRows", { sheet, column })
        : t("manage.locator.cells", { sheet, column, rows: rows.join(", ") });
    }
    case "filename":
      return t("manage.locator.filename");
    case "metadata":
      return t("manage.locator.metadata");
    default:
      return t("manage.locator.unknown");
  }
}

/** What the strip under the title says about the documents the next Analyse will read again. */
export function pendingNote(t: Translator, documentsDue: number): string | null {
  if (documentsDue <= 0) {
    return null;
  }
  return documentsDue === 1
    ? t("manage.pendingOne", { count: documentsDue })
    : t("manage.pendingMany", { count: documentsDue });
}

export function summaryLine(t: Translator, overview: KnowledgeOverview): string {
  return t("manage.summary", {
    entities: counted(
      overview.entitiesLive,
      t("manage.countEntityOne"),
      t("manage.countEntityMany"),
    ),
    mentions: counted(
      overview.mentions,
      t("manage.countMentionOne"),
      t("manage.countMentionMany"),
    ),
    documents: counted(
      overview.documentsRead,
      t("manage.countDocumentOne"),
      t("manage.countDocumentMany"),
    ),
    workbooks: counted(
      overview.workbooksRead,
      t("manage.countWorkbookOne"),
      t("manage.countWorkbookMany"),
    ),
  });
}

/** The operation's name for the undo button, or the generic one for a kind this build does not know. */
export function operationLabel(t: Translator, kind: string): string {
  return labelled(t, "manage.operation", kind, "other");
}

/** The undo button's label and whether it can be pressed. */
export function undoState(
  t: Translator,
  last: LastOperation | null,
): { label: string; enabled: boolean } {
  if (last === null) {
    return { label: t("manage.undoNone"), enabled: false };
  }
  const operation = operationLabel(t, last.kind);
  return last.undoable
    ? { label: t("manage.undoButton", { operation }), enabled: true }
    : { label: t("manage.undoBlocked", { operation }), enabled: false };
}

/** A typed name under three letters is found in documents but never read in a question. */
export function isShortName(text: string): boolean {
  return [...text].filter((character) => /\p{L}|\p{N}/u.test(character)).length < 3;
}

/**
 * Which entity a merge keeps when the user has not said: the one the base saw more often, then the
 * older one (the smaller id), so the choice is the same on every screen. Only a default: she can
 * swap them before confirming.
 */
export function defaultSurvivor(
  one: { entityId: number; mentions: number },
  other: { entityId: number; mentions: number },
): { survivor: number; absorbed: number } {
  const keepOne =
    one.mentions !== other.mentions ? one.mentions > other.mentions : one.entityId < other.entityId;
  return keepOne
    ? { survivor: one.entityId, absorbed: other.entityId }
    : { survivor: other.entityId, absorbed: one.entityId };
}

/** Only an entity of the same type, different from the one being merged, can be the other side. */
export function canMergeWith(entity: EntityRow, candidate: EntityRow): boolean {
  return (
    candidate.entityId !== entity.entityId &&
    candidate.typeId === entity.typeId &&
    candidate.status !== "deleted" &&
    candidate.status !== "merged"
  );
}

export function toggleId(ids: readonly number[], id: number): number[] {
  return ids.includes(id) ? ids.filter((known) => known !== id) : [...ids, id];
}

/** A split moves some of an entity's mentions, never none and never all of them. */
export function canSplit(picked: number, total: number, name: string): boolean {
  return picked >= 1 && picked < total && name.trim() !== "";
}

/** The suggestion offered in the split form: the name with a number, so it differs from every other. */
export function suggestedSplitName(name: string): string {
  return `${name} 2`;
}

export type GuardedAction =
  | "delete"
  | "merge"
  | "split"
  | "removeAlias"
  | "removeAttribute"
  | "reset"
  | "resetAll";

/** A confirmation waiting for its answer: what is asked, the values its sentences use, and what to
 * do when the answer is yes. */
export interface PendingConfirm {
  action: GuardedAction;
  values: Record<string, string | number>;
  apply(): Promise<void>;
}

export interface Confirmation {
  /** Catalogue prefix; the dialog reads `<prefix>.title`, `.body` and `.confirm`. */
  prefix: string;
  destructive: boolean;
}

/**
 * Every action that removes, redirects or moves something goes through the confirmation dialog.
 * Adding, editing, restoring, unmerging and undoing do not: each of them is undone by one click.
 */
export function confirmationFor(action: GuardedAction): Confirmation {
  return { prefix: `manage.confirm.${action}`, destructive: action !== "merge" && action !== "split" };
}

const UNGUARDED: readonly string[] = [
  "create",
  "edit",
  "addAlias",
  "setAttribute",
  "restore",
  "unmerge",
  "undo",
  "dismissMatch",
];

/** The list of actions that act at once, kept here so a test can say which ones those are. */
export function actsAtOnce(action: string): boolean {
  return UNGUARDED.includes(action);
}

/** A busy database is retried, not reported as a fault. */
export function isBusy(code: string): boolean {
  return code === "knowledge_unavailable";
}
