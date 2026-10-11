import type { Translator } from "../i18n/translate";
import type { EntityCard } from "./ipc";
import { predicateLabel, roleLabel, subtypeLabel, typeLabel } from "./knowledgeManage";

/**
 * The entity sheet of a selection, written for the screen (`knowledge::sheet`).
 *
 * Rust sends names, paths, counts and machine codes; every sentence is written here. Files outside
 * the selection are only ever a count, and the sheet never offers to change the selection: that is
 * the labelled "add to my selection" offer of the name list, which is the user's own click.
 */

export interface CardLine {
  key: string;
  text: string;
}

export interface CardView {
  title: string;
  /** "Person", "Person - client"... : the type and, when the build labels it, the subtype. */
  kind: string;
  unconfirmed: boolean;
  aliases: string[];
  roles: string[];
  documents: CardLine[];
  tables: CardLine[];
  /** Nothing in the selection holds the name. */
  nowhere: boolean;
  outside: string | null;
  relations: CardLine[];
  details: CardLine[];
}

function mentions(t: Translator, count: number): string {
  return count === 1 ? t("entityCard.mentionOne") : t("entityCard.mentionMany", { count });
}

function rows(t: Translator, count: number): string {
  return count === 1 ? t("entityCard.rowOne") : t("entityCard.rowMany", { count });
}

export function cardView(t: Translator, card: EntityCard): CardView {
  const subtype = subtypeLabel(t, card.subtype);
  const type = typeLabel(t, card.typeId);
  const outsideParts: string[] = [];
  if (card.outside.documents > 0) {
    outsideParts.push(t("entityCard.outsideDocuments", { count: card.outside.documents }));
  }
  if (card.outside.workbooks > 0) {
    outsideParts.push(t("entityCard.outsideWorkbooks", { count: card.outside.workbooks }));
  }
  return {
    title: t("entityCard.title", { name: card.display }),
    kind: subtype === null ? type : t("entityCard.kindWithSubtype", { type, subtype }),
    unconfirmed: card.status === "candidate",
    aliases: card.aliases,
    roles: card.roles.map((role) =>
      t("entityCard.roleLine", {
        role: roleLabel(t, role.role) ?? role.role,
        mentions: mentions(t, role.mentions),
      }),
    ),
    documents: card.documents.map((document) => {
      const base = { path: document.relativePath, mentions: mentions(t, document.mentions) };
      return {
        key: document.relativePath,
        text:
          document.pages.length === 0
            ? t("entityCard.documentLine", base)
            : t("entityCard.documentLinePages", {
                ...base,
                pages: t(document.pages.length === 1 ? "entityCard.pageOne" : "entityCard.pageMany", {
                  pages: document.pages.join(", "),
                }),
              }),
      };
    }),
    tables: card.tables.map((table) => ({
      key: `${table.relativePath}|${table.sheet}|${table.column}`,
      text: t("entityCard.tableLine", {
        path: table.relativePath,
        sheet: table.sheet,
        column: table.column,
        rows: rows(t, table.rows),
      }),
    })),
    nowhere: card.documents.length === 0 && card.tables.length === 0,
    outside:
      outsideParts.length === 0
        ? null
        : t("entityCard.outside", { files: outsideParts.join(", ") }),
    relations: card.relations.map((relation) => {
      const predicate = predicateLabel(t, relation.predicate);
      const key = relation.direction === "subject" ? "relationSubject" : "relationObject";
      return {
        key: `${relation.predicate}|${relation.direction}|${relation.otherEntityId}`,
        text: t(`entityCard.${key}`, {
          name: card.display,
          predicate,
          other: relation.otherDisplay,
        }),
      };
    }),
    details: card.attributes.map((attribute) => ({
      key: `${attribute.key}|${attribute.value}`,
      text: t("entityCard.detailLine", { key: attribute.key, value: attribute.value }),
    })),
  };
}
