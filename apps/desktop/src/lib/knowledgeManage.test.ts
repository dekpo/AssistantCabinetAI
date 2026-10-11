import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, lookupKey, type Catalogue } from "../i18n/translate";
import { errorMessage } from "./errors";
import type { EntityRow, KnowledgeOverview, LocatorView } from "./ipc";
import {
  actsAtOnce,
  aliasKindLabel,
  canMergeWith,
  canSplit,
  clampPage,
  columnReasonLabel,
  confidenceText,
  confirmationFor,
  defaultSurvivor,
  EMPTY_FILTERS,
  entityTitle,
  isBusy,
  isShortName,
  listQueryFor,
  locatorText,
  methodLabel,
  operationLabel,
  pageCount,
  pendingNote,
  predicateLabel,
  reasonLabel,
  roleLabel,
  subtypeLabel,
  suggestedSplitName,
  summaryLine,
  toggleId,
  typeLabel,
  undoState,
  withFilter,
  type GuardedAction,
} from "./knowledgeManage";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

const HERE = fileURLToPath(new URL(".", import.meta.url));
const RUST = resolve(HERE, "../../src-tauri");

function row(overrides: Partial<EntityRow> = {}): EntityRow {
  return {
    entityId: 1,
    typeId: "person",
    subtype: null,
    name: "Jean Dupont",
    disambiguator: "",
    status: "active",
    origin: "automatic",
    mergedInto: null,
    mentions: 2,
    sources: 2,
    aliases: 3,
    openMatches: 0,
    ...overrides,
  };
}

function locator(overrides: Partial<LocatorView>): LocatorView {
  return {
    kind: "chunk",
    page: null,
    section: null,
    sheet: null,
    column: null,
    firstRow: null,
    sampleRows: [],
    ...overrides,
  };
}

describe("the list query", () => {
  it("sends nothing for a filter left at its empty value", () => {
    expect(listQueryFor(EMPTY_FILTERS, 0)).toStrictEqual({
      text: "",
      typeId: null,
      origin: null,
      view: "live",
      problemsOnly: false,
      page: 0,
    });
  });

  it("trims the text and keeps the chosen filters", () => {
    const filters = withFilter(EMPTY_FILTERS, {
      text: "  dup ",
      typeId: "organization",
      origin: "manual",
      view: "deleted",
      problemsOnly: true,
    });

    expect(listQueryFor(filters, 3)).toStrictEqual({
      text: "dup",
      typeId: "organization",
      origin: "manual",
      view: "deleted",
      problemsOnly: true,
      page: 3,
    });
  });

  it("never asks for a negative or fractional page", () => {
    expect(listQueryFor(EMPTY_FILTERS, -2).page).toBe(0);
    expect(listQueryFor(EMPTY_FILTERS, 1.9).page).toBe(1);
  });

  it("does not change the filters it was given", () => {
    const before = { ...EMPTY_FILTERS };
    withFilter(EMPTY_FILTERS, { text: "x" });

    expect(EMPTY_FILTERS).toStrictEqual(before);
  });
});

describe("pages", () => {
  it("counts at least one page, even for an empty list", () => {
    expect(pageCount(0, 50)).toBe(1);
    expect(pageCount(50, 50)).toBe(1);
    expect(pageCount(51, 50)).toBe(2);
    expect(pageCount(120, 50)).toBe(3);
    expect(pageCount(10, 0)).toBe(1);
  });

  it("brings a page back inside the list after a deletion emptied the last one", () => {
    expect(clampPage(2, 100, 50)).toBe(1);
    expect(clampPage(5, 0, 50)).toBe(0);
    expect(clampPage(-1, 100, 50)).toBe(0);
    expect(clampPage(1, 100, 50)).toBe(1);
  });
});

describe("names", () => {
  it("shows the disambiguator beside the name", () => {
    expect(entityTitle({ name: "Jean Dupont", disambiguator: "" })).toBe("Jean Dupont");
    expect(entityTitle({ name: "Jean Dupont", disambiguator: "Lyon" })).toBe("Jean Dupont (Lyon)");
  });

  it("suggests a new name that differs from the old one", () => {
    expect(suggestedSplitName("Jean Dupont")).toBe("Jean Dupont 2");
  });

  it("calls a name under three letters short, whatever its alphabet", () => {
    expect(isShortName("JD")).toBe(true);
    expect(isShortName("J. D.")).toBe(true);
    expect(isShortName("Roy")).toBe(false);
    expect(isShortName("  ")).toBe(true);
    expect(isShortName("Éa")).toBe(true);
    expect(isShortName("Zoé")).toBe(false);
  });
});

describe("where a mention was found, in words", () => {
  it("reads a page and a passage", () => {
    const place = locator({ page: 3, section: 2 });

    expect(locatorText(french, place)).toBe("page 3, passage 2");
    expect(locatorText(english, place)).toBe("page 3, passage 2");
  });

  it("reads a page alone, and an unknown place", () => {
    expect(locatorText(french, locator({ page: 4 }))).toBe("page 4");
    expect(locatorText(french, locator({}))).toBe("emplacement inconnu");
  });

  it("reads a sheet, a column and the rows kept", () => {
    const place = locator({
      kind: "cells",
      sheet: "Mars",
      column: "Medecin",
      firstRow: 2,
      sampleRows: [2, 5, 9],
    });

    expect(locatorText(french, place)).toBe("feuille « Mars », colonne « Medecin », lignes 2, 5, 9");
    expect(locatorText(english, place)).toBe("sheet “Mars”, column “Medecin”, rows 2, 5, 9");
  });

  it("falls back to the first row, and then to the column alone", () => {
    expect(locatorText(english, locator({ kind: "cells", sheet: "S", column: "C", firstRow: 7 }))).toBe(
      "sheet “S”, column “C”, rows 7",
    );
    expect(locatorText(english, locator({ kind: "cells", sheet: "S", column: "C" }))).toBe(
      "sheet “S”, column “C”",
    );
  });

  it("reads a file name and a file's information", () => {
    expect(locatorText(french, locator({ kind: "filename" }))).toBe("dans le nom du fichier");
    expect(locatorText(english, locator({ kind: "metadata" }))).toBe("in the file information");
  });

  it("writes a confidence as a whole percentage, kept inside 0 to 100", () => {
    expect(confidenceText(0.904)).toBe("90 %");
    expect(confidenceText(1.4)).toBe("100 %");
    expect(confidenceText(-1)).toBe("0 %");
  });
});

describe("the strip under the title", () => {
  const overview: KnowledgeOverview = {
    entitiesLive: 12,
    entitiesCandidate: 2,
    entitiesManual: 1,
    entitiesMerged: 0,
    entitiesDeleted: 0,
    aliases: 30,
    mentions: 1,
    documentsRead: 30,
    workbooksRead: 0,
    openPossibleMatches: 0,
    documentsDue: 0,
    consistent: true,
    lastOperation: null,
  };

  it("counts with singular and plural, in both languages", () => {
    expect(summaryLine(french, overview)).toBe("12 entités, 1 mention, 30 documents lus, 0 tableur lu.");
    expect(summaryLine(english, overview)).toBe(
      "12 entities, 1 mention, 30 documents read, 0 spreadsheet read.",
    );
  });

  it("says nothing when no document is due, and says how many otherwise", () => {
    expect(pendingNote(french, 0)).toBeNull();
    expect(pendingNote(french, 1)).toContain("1 document sera relu");
    expect(pendingNote(french, 4)).toContain("4 documents seront relus");
    expect(pendingNote(english, 4)).toContain("4 documents will be read again");
    // The reading is local: the sentence says the AI is not asked.
    expect(pendingNote(french, 2)).toContain("l'IA n'est pas sollicitée");
  });

  it("names the operation the undo button would reverse", () => {
    expect(undoState(french, { opId: 1, kind: "merge", undoable: true, at: 1 })).toStrictEqual({
      label: "Annuler : la fusion",
      enabled: true,
    });
    expect(undoState(english, { opId: 1, kind: "split", undoable: true, at: 1 }).label).toBe(
      "Undo: the split",
    );
  });

  it("disables the button when there is nothing to undo or the last operation cannot be", () => {
    expect(undoState(french, null)).toStrictEqual({ label: "Rien à annuler", enabled: false });
    const blocked = undoState(french, { opId: 2, kind: "remove_alias", undoable: false, at: 1 });
    expect(blocked.enabled).toBe(false);
    expect(blocked.label).toContain("le retrait d'un nom");
  });

  it("uses a generic name for an operation this build does not know", () => {
    expect(operationLabel(french, "teleport")).toBe("la dernière opération");
  });
});

describe("which entity a merge keeps", () => {
  it("keeps the one the base saw more often", () => {
    expect(defaultSurvivor({ entityId: 1, mentions: 2 }, { entityId: 2, mentions: 9 })).toStrictEqual({
      survivor: 2,
      absorbed: 1,
    });
  });

  it("keeps the older one when they are seen as often", () => {
    expect(defaultSurvivor({ entityId: 8, mentions: 3 }, { entityId: 4, mentions: 3 })).toStrictEqual({
      survivor: 4,
      absorbed: 8,
    });
    expect(defaultSurvivor({ entityId: 4, mentions: 3 }, { entityId: 8, mentions: 3 })).toStrictEqual({
      survivor: 4,
      absorbed: 8,
    });
  });

  it("offers only another live entity of the same type", () => {
    const entity = row({ entityId: 1 });

    expect(canMergeWith(entity, row({ entityId: 2 }))).toBe(true);
    expect(canMergeWith(entity, row({ entityId: 1 }))).toBe(false);
    expect(canMergeWith(entity, row({ entityId: 3, typeId: "organization" }))).toBe(false);
    expect(canMergeWith(entity, row({ entityId: 4, status: "deleted" }))).toBe(false);
    expect(canMergeWith(entity, row({ entityId: 5, status: "merged" }))).toBe(false);
    expect(canMergeWith(entity, row({ entityId: 6, status: "candidate" }))).toBe(true);
  });
});

describe("a split", () => {
  it("moves some of the mentions, never none and never all", () => {
    expect(canSplit(0, 4, "Jean Dupont 2")).toBe(false);
    expect(canSplit(1, 4, "Jean Dupont 2")).toBe(true);
    expect(canSplit(3, 4, "Jean Dupont 2")).toBe(true);
    expect(canSplit(4, 4, "Jean Dupont 2")).toBe(false);
    expect(canSplit(1, 4, "   ")).toBe(false);
  });

  it("ticks and unticks a mention without changing the list it was given", () => {
    const start = [1, 2];

    expect(toggleId(start, 3)).toStrictEqual([1, 2, 3]);
    expect(toggleId(start, 2)).toStrictEqual([1]);
    expect(start).toStrictEqual([1, 2]);
  });
});

describe("what asks before it acts", () => {
  const guarded: GuardedAction[] = [
    "delete",
    "merge",
    "split",
    "removeAlias",
    "removeAttribute",
    "reset",
    "resetAll",
  ];

  it.each(guarded)("%s has a title, a body and a button in both languages", (action) => {
    const { prefix } = confirmationFor(action);

    for (const locale of ["fr-FR", "en-US"]) {
      const catalogue = CATALOGUES[locale] as Catalogue;
      for (const part of ["title", "body", "confirm"]) {
        expect(lookupKey(catalogue, `${prefix}.${part}`), `${locale} ${prefix}.${part}`).toBeDefined();
      }
    }
  });

  it("marks what removes something as destructive, and a merge or a split as a decision", () => {
    expect(confirmationFor("delete").destructive).toBe(true);
    expect(confirmationFor("removeAlias").destructive).toBe(true);
    expect(confirmationFor("reset").destructive).toBe(true);
    expect(confirmationFor("resetAll").destructive).toBe(true);
    expect(confirmationFor("merge").destructive).toBe(false);
    expect(confirmationFor("split").destructive).toBe(false);
  });

  it("acts at once only on what one click undoes", () => {
    for (const action of ["create", "edit", "addAlias", "restore", "unmerge", "undo"]) {
      expect(actsAtOnce(action), action).toBe(true);
    }
    for (const action of guarded) {
      expect(actsAtOnce(action), action).toBe(false);
    }
  });

  it("fills the sentences with the names it was given", () => {
    const merge = confirmationFor("merge").prefix;

    expect(french(`${merge}.body`, { absorbed: "J. Dupont", survivor: "Jean Dupont" })).toContain(
      "« J. Dupont » devient un autre nom de « Jean Dupont »",
    );
    expect(french(`${confirmationFor("delete").prefix}.title`, { name: "Faux Nom" })).toBe(
      "Supprimer « Faux Nom » ?",
    );
  });
});

describe("the words of every machine code the base can send", () => {
  /** The `=> "code"` arms of `fn as_code` inside `impl <name>` of a Rust source. */
  function codesOf(path: string, implName: string): string[] {
    const source = readFileSync(resolve(RUST, path), "utf8").split(String.fromCharCode(13, 10)).join(String.fromCharCode(10));
    const start = source.indexOf(`impl ${implName} {`);
    expect(start, `impl ${implName} in ${path}`).toBeGreaterThanOrEqual(0);
    const body = source.slice(start);
    const fn = body.indexOf("fn as_code");
    expect(fn, `as_code in impl ${implName}`).toBeGreaterThanOrEqual(0);
    const block = body.slice(fn, body.indexOf("\n    }\n", fn));
    return [...block.matchAll(/=>\s*"([a-z_]+)"/g)].map(([, code]) => code as string);
  }

  it("labels every extraction method", () => {
    const codes = codesOf("src/knowledge/mod.rs", "Method");

    expect(codes.length).toBeGreaterThan(8);
    for (const code of codes) {
      for (const t of [french, english]) {
        expect(t(`manage.method.${code}`), code).not.toBe(`manage.method.${code}`);
      }
      expect(methodLabel(french, code)).not.toBe(methodLabel(french, "no_such_method"));
    }
  });

  it("labels every kind of alias", () => {
    const codes = codesOf("src/knowledge/mod.rs", "AliasKind");

    expect(codes).toContain("partial_surname");
    for (const code of codes) {
      expect(french(`manage.aliasKind.${code}`), code).not.toBe(`manage.aliasKind.${code}`);
      expect(english(`manage.aliasKind.${code}`), code).not.toBe(`manage.aliasKind.${code}`);
    }
    expect(aliasKindLabel(french, "unknown_kind")).toBe("Ajouté à la main");
  });

  it("labels every status and every origin", () => {
    for (const code of codesOf("src/knowledge/mod.rs", "EntityStatus")) {
      expect(french(`manage.status.${code}`), code).not.toBe(`manage.status.${code}`);
    }
    for (const code of codesOf("src/knowledge/mod.rs", "Origin")) {
      expect(french(`manage.origin.${code}`), code).not.toBe(`manage.origin.${code}`);
    }
  });

  it("labels every reason a pair is offered and a column was read or not", () => {
    for (const code of codesOf("src/knowledge/resolve.rs", "PossibleReason")) {
      expect(french(`manage.reason.${code}`), code).not.toBe(`manage.reason.${code}`);
    }
    const tableSource = readFileSync(resolve(RUST, "src/knowledge/extract/table.rs"), "utf8");
    const reasons = [...tableSource.matchAll(/Self::\w+ => "([a-z_]+)",/g)].map(([, code]) => code as string);
    expect(reasons).toContain("header_exact");
    for (const code of reasons) {
      expect(french(`manage.columnReason.${code}`), code).not.toBe(`manage.columnReason.${code}`);
      expect(english(`manage.columnReason.${code}`), code).not.toBe(`manage.columnReason.${code}`);
    }
    expect(reasonLabel(french, "no_such_reason")).toBe("ils se ressemblent");
    expect(columnReasonLabel(french, "no_such_reason")).toBe("non précisé");
  });

  it("labels every predicate and every subtype the packs can produce", () => {
    const packs = resolve(RUST, "resources/knowledge");
    const bodies: string[] = [];
    const walk = (directory: string) => {
      for (const entry of readdirSync(directory, { withFileTypes: true })) {
        const path = resolve(directory, entry.name);
        if (entry.isDirectory()) {
          walk(path);
        } else if (entry.name.endsWith(".json")) {
          bodies.push(readFileSync(path, "utf8"));
        }
      }
    };
    walk(packs);
    const text = bodies.join("\n");
    const predicates = new Set([...text.matchAll(/"predicate":\s*"([a-z_]+)"/g)].map(([, c]) => c as string));
    const subtypes = new Set([...text.matchAll(/"subtype":\s*"([a-z_]+)"/g)].map(([, c]) => c as string));

    expect(predicates.size).toBeGreaterThan(2);
    expect(subtypes.size).toBeGreaterThan(10);
    for (const code of predicates) {
      expect(french(`manage.predicate.${code}`), code).not.toBe(`manage.predicate.${code}`);
      expect(predicateLabel(english, code)).not.toBe(predicateLabel(english, "no_such_predicate"));
    }
    for (const code of subtypes) {
      expect(subtypeLabel(french, code), code).not.toBeNull();
      expect(subtypeLabel(english, code), code).not.toBeNull();
    }
    expect(subtypeLabel(french, "no_such_subtype")).toBeNull();
    expect(subtypeLabel(french, null)).toBeNull();
  });

  it("labels the six built-in types and every role the base defines", () => {
    for (const code of ["person", "organization", "location", "identifier", "item", "term"]) {
      expect(typeLabel(french, code), code).not.toBe("");
      expect(typeLabel(english, code), code).not.toBe("");
    }
    expect(typeLabel(french, "no_such_type")).toBe("");
    expect(roleLabel(french, "client")).toBe("Client");
    expect(roleLabel(french, "no_such_role")).toBeNull();
    expect(roleLabel(french, null)).toBeNull();
  });
});

describe("failures", () => {
  it("has a sentence for every refusal the management operations can give", () => {
    for (const code of [
      "knowledge_unavailable",
      "knowledge_entity_not_found",
      "knowledge_merge_refused",
      "knowledge_name_invalid",
      "knowledge_name_taken",
      "knowledge_operation_refused",
      "knowledge_nothing_to_undo",
    ]) {
      const message = errorMessage(french, { code, data: {} });

      expect(message, code).not.toBe(french("errors.unknown_error"));
      expect(errorMessage(english, { code, data: {} }), code).not.toBe(english("errors.unknown_error"));
    }
  });

  it("says nothing was changed when a name is refused", () => {
    expect(errorMessage(french, { code: "knowledge_name_taken", data: {} })).toContain("Rien n'a été modifié");
    expect(errorMessage(french, { code: "knowledge_operation_refused", data: {} })).toContain(
      "rien n'a été modifié",
    );
  });

  it("retries a busy database and nothing else", () => {
    expect(isBusy("knowledge_unavailable")).toBe(true);
    expect(isBusy("knowledge_name_taken")).toBe(false);
  });

  it("never calls the AI a server in what it shows", () => {
    for (const locale of ["fr-FR", "en-US"]) {
      const text = JSON.stringify((CATALOGUES[locale] as Catalogue).manage).toLowerCase();
      expect(text, locale).not.toMatch(/\bserveur\b|\bserver\b/);
    }
  });
});
