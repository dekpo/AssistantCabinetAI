import type { EntryFacts, ExplorerEntry, ExplorerRoot } from "./explorerModel";
import { explorerKey } from "./explorerModel";
import type { FactEntity, FolderKnowledgeFacts } from "./ipc";

/*
 * The knowledge-base facts of the explorer, with no React and no call: the facts of the two folders
 * joined by `explorerKey`, attached to the entries, and the names of the people and organisations they
 * link to. The facts are read once per folder (`file_knowledge_facts`); every count and every filter of
 * the knowledge base is then worked out here, so a filter change costs no call (`docs/KNOWLEDGE-BASE.md`,
 * "Filters of the explorer"). A person stays a person and an organisation an organisation: the type
 * comes from the base, never from the active packs.
 */

/** The facts of both folders, joined. */
export interface KnowledgeFacts {
  byKey: ReadonlyMap<string, EntryFacts>;
  /** The people and organisations the files link to, by id. */
  entities: ReadonlyMap<number, FactEntity>;
}

export const NO_FACTS: KnowledgeFacts = { byKey: new Map(), entities: new Map() };

/** The facts of the two folders as one value; a folder not read (null) adds nothing. */
export function joinFacts(
  folders: Readonly<Record<ExplorerRoot, FolderKnowledgeFacts | null>>,
): KnowledgeFacts {
  const byKey = new Map<string, EntryFacts>();
  const entities = new Map<number, FactEntity>();
  for (const root of ["documents", "data"] as const) {
    const facts = folders[root];
    if (facts === null) {
      continue;
    }
    for (const file of facts.files) {
      byKey.set(explorerKey(root, file.relativePath), {
        type: file.businessType,
        years: file.years,
        links: file.entities,
      });
    }
    for (const entity of facts.entities) {
      entities.set(entity.entityId, entity);
    }
  }
  return { byKey, entities };
}

/** The entries with their facts attached; an entry the facts do not know is returned as it is. */
export function attachFacts(
  entries: readonly ExplorerEntry[],
  facts: KnowledgeFacts,
): ExplorerEntry[] {
  if (facts.byKey.size === 0) {
    return [...entries];
  }
  return entries.map((entry) => {
    const found = facts.byKey.get(entry.key);
    return found === undefined ? entry : { ...entry, facts: found };
  });
}

/** The two kinds of name the panel lists. */
export type EntityKind = "person" | "organisation";

/** The type code the base gives each kind. */
export const ENTITY_TYPE: Readonly<Record<EntityKind, string>> = {
  person: "person",
  organisation: "organization",
};
