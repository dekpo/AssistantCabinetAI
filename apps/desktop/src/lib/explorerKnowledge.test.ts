import { describe, expect, it } from "vitest";
import { attachFacts, joinFacts, NO_FACTS, type KnowledgeFacts } from "./explorerFacts";
import {
  activeFilterCount,
  activePills,
  chosenEntityIds,
  entityOptions,
  NO_CHOICES,
  removePill,
  setEntityRole,
  suggestEntities,
  toggleChoice,
  typeChoices,
  yearChoices,
} from "./explorerFilters";
import {
  entityChoice,
  filterEntries,
  mergeInventories,
  parseEntityChoice,
  type ExplorerEntry,
  type ExplorerFilter,
} from "./explorerModel";
import { isFiltering } from "./explorerSelection";
import type { FileRecord, FolderKnowledgeFacts } from "./ipc";

function record(relativePath: string, status: "indexed" | "discovered" = "indexed"): FileRecord {
  const name = relativePath.split("/").pop() ?? relativePath;
  const dot = name.lastIndexOf(".");
  return {
    id: `id-${relativePath}`,
    relativePath,
    name,
    stem: name.slice(0, dot),
    extension: name.slice(dot + 1),
    sizeBytes: 1,
    modifiedAt: 1,
    processingStatus: status,
  } as FileRecord;
}

/* Documents folder: five files. Alice (person 1) is the subject of a.pdf and the author of b.pdf, and is
   only mentioned in c.pdf. Bob (person 2) is the subject of c.pdf. Acme (organisation 10) issued a.pdf
   and b.pdf; Beta (organisation 11) issued c.pdf. d.pdf is analysed with no fact at all, e.pdf is not
   analysed. Data folder: one workbook spanning 2025 and 2026. */
const documentFacts: FolderKnowledgeFacts = {
  files: [
    {
      relativePath: "a.pdf",
      businessType: "invoice",
      years: ["2026"],
      entities: [
        { entityId: 1, roles: ["subject"] },
        { entityId: 10, roles: ["issuer"] },
      ],
    },
    {
      relativePath: "b.pdf",
      businessType: "letter",
      years: ["2025"],
      entities: [
        { entityId: 1, roles: ["author"] },
        { entityId: 10, roles: ["issuer", "mention"] },
      ],
    },
    {
      relativePath: "c.pdf",
      businessType: "invoice",
      years: ["2025", "2026"],
      entities: [
        { entityId: 1, roles: ["mention"] },
        { entityId: 2, roles: ["subject"] },
        { entityId: 11, roles: ["issuer"] },
      ],
    },
  ],
  entities: [
    { entityId: 1, name: "Alice Martin", typeId: "person" },
    { entityId: 2, name: "Bob Durand", typeId: "person" },
    { entityId: 10, name: "Acme SA", typeId: "organization" },
    { entityId: 11, name: "Beta Assurances", typeId: "organization" },
  ],
};
const dataFacts: FolderKnowledgeFacts = {
  files: [
    {
      relativePath: "grid.xlsx",
      businessType: "invoicing",
      years: ["2025", "2026"],
      entities: [{ entityId: 1, roles: ["client"] }],
    },
  ],
  entities: [{ entityId: 1, name: "Alice Martin", typeId: "person" }],
};

const facts: KnowledgeFacts = joinFacts({ documents: documentFacts, data: dataFacts });
const entries: ExplorerEntry[] = attachFacts(
  mergeInventories(
    [record("a.pdf"), record("b.pdf"), record("c.pdf"), record("d.pdf"), record("e.pdf", "discovered")],
    [record("grid.xlsx")],
  ).entries,
  facts,
);
const names = (list: readonly ExplorerEntry[]) => list.map((entry) => entry.name).sort();

describe("the facts joined to the entries", () => {
  it("are attached by folder and path, so the same path in both folders stays two files", () => {
    const both = attachFacts(
      mergeInventories([record("x.pdf")], [record("x.pdf")]).entries,
      joinFacts({
        documents: {
          files: [{ relativePath: "x.pdf", businessType: "letter", years: [], entities: [] }],
          entities: [],
        },
        data: null,
      }),
    );

    expect(both.map((entry) => entry.facts?.type ?? null)).toStrictEqual(["letter", null]);
  });

  it("place the facts of a file read in the other folder by the folder its key names", () => {
    // A document of the Data folder is a file of the documents family stored as `/data/...`; a workbook of
    // the Documents folder is a file of the tables family stored as `/documents/...`.
    const placed = attachFacts(
      mergeInventories([record("x.xlsx"), record("x.pdf")], [record("x.pdf"), record("y.pdf")]).entries,
      joinFacts({
        documents: {
          files: [
            { relativePath: "x.pdf", businessType: "letter", years: [], entities: [] },
            { relativePath: "/data/x.pdf", businessType: "invoice", years: ["2026"], entities: [] },
          ],
          entities: [],
        },
        data: {
          files: [
            { relativePath: "/documents/x.xlsx", businessType: "register", years: [], entities: [] },
          ],
          entities: [],
        },
      }),
    );

    const byKey = Object.fromEntries(placed.map((entry) => [entry.key, entry.facts?.type ?? null]));
    expect(byKey).toStrictEqual({
      "documents:x.xlsx": "register",
      "documents:x.pdf": "letter",
      "data:x.pdf": "invoice",
      "data:y.pdf": null,
    });
  });

  it("leave a file the facts do not know without any", () => {
    const d = entries.find((entry) => entry.name === "d.pdf");

    expect(d?.facts).toBeUndefined();
    expect(attachFacts(entries, NO_FACTS)).toHaveLength(entries.length);
  });

  it("know the people and organisations of both folders by id", () => {
    expect(facts.entities.get(1)?.name).toBe("Alice Martin");
    expect(facts.entities.get(11)?.typeId).toBe("organization");
  });
});

describe("filtering by what the knowledge base knows", () => {
  it("keeps the files linked to a person, whatever the role", () => {
    const result = filterEntries(entries, { people: new Set([entityChoice(1)]) });

    expect(names(result)).toStrictEqual(["a.pdf", "b.pdf", "c.pdf", "grid.xlsx"]);
  });

  it("keeps only the files where the person has the role asked for", () => {
    const author = filterEntries(entries, { people: new Set([entityChoice(1, "author")]) });
    const client = filterEntries(entries, { people: new Set([entityChoice(1, "client")]) });

    expect(names(author)).toStrictEqual(["b.pdf"]);
    expect(names(client)).toStrictEqual(["grid.xlsx"]);
  });

  it("reads two people as this or that", () => {
    const result = filterEntries(entries, {
      people: new Set([entityChoice(2), entityChoice(1, "author")]),
    });

    expect(names(result)).toStrictEqual(["b.pdf", "c.pdf"]);
  });

  it("combines a person, an organisation, a type and a year as this and that", () => {
    const result = filterEntries(entries, {
      people: new Set([entityChoice(1)]),
      organisations: new Set([entityChoice(10)]),
      types: new Set(["invoice"]),
      years: new Set(["2026"]),
    });

    expect(names(result)).toStrictEqual(["a.pdf"]);
  });

  it("counts a file in every year it spans", () => {
    expect(names(filterEntries(entries, { years: new Set(["2025"]) }))).toStrictEqual([
      "b.pdf",
      "c.pdf",
      "grid.xlsx",
    ]);
  });

  it("passes no filter of the knowledge base for a file nothing is known of", () => {
    const parts: ExplorerFilter[] = [
      { people: new Set([entityChoice(1)]) },
      { organisations: new Set([entityChoice(10)]) },
      { types: new Set(["invoice"]) },
      { years: new Set(["2026"]) },
    ];
    for (const part of parts) {
      const kept = names(filterEntries(entries, part));
      expect(kept).not.toContain("d.pdf");
      expect(kept).not.toContain("e.pdf");
    }
  });

  it("combines with the text found and the family", () => {
    const found = new Set(
      entries.filter((e) => ["a.pdf", "c.pdf", "grid.xlsx"].includes(e.name)).map((e) => e.key),
    );
    const result = filterEntries(entries, {
      people: new Set([entityChoice(1)]),
      found,
      family: "documents",
    });

    expect(names(result)).toStrictEqual(["a.pdf", "c.pdf"]);
  });

  it("keeps only the files of the conversation while that filter is on", () => {
    const ticked = new Set(entries.filter((e) => e.name === "b.pdf").map((e) => e.key));

    expect(names(filterEntries(entries, { ticked }))).toStrictEqual(["b.pdf"]);
    expect(filterEntries(entries, { ticked: null })).toHaveLength(entries.length);
    expect(filterEntries(entries, { ticked: new Set() })).toHaveLength(0);
  });

  it("round-trips an entity choice", () => {
    expect(parseEntityChoice(entityChoice(12))).toStrictEqual({ entityId: 12, role: null });
    expect(parseEntityChoice(entityChoice(12, "author"))).toStrictEqual({
      entityId: 12,
      role: "author",
    });
  });
});

describe("the counts beside the choices", () => {
  it("count the business types over what the other filters leave, the own choice left out", () => {
    expect(typeChoices(entries, {})).toStrictEqual([
      { code: "invoice", count: 2 },
      { code: "invoicing", count: 1 },
      { code: "letter", count: 1 },
    ]);

    // The type chosen does not narrow its own list; the year does: b.pdf is 2025 only.
    const withYear = typeChoices(entries, { years: new Set(["2026"]), types: new Set(["letter"]) });
    expect(withYear).toStrictEqual([
      { code: "invoice", count: 2 },
      { code: "invoicing", count: 1 },
      { code: "letter", count: 0 },
    ]);
  });

  it("list the years newest first, a file spanning two years in both", () => {
    expect(yearChoices(entries, {})).toStrictEqual([
      { year: "2026", count: 3 },
      { year: "2025", count: 3 },
    ]);
    expect(yearChoices(entries, { types: new Set(["letter"]) })).toStrictEqual([
      { year: "2026", count: 0 },
      { year: "2025", count: 1 },
    ]);
  });

  it("keep a choice that would empty the list and say 0", () => {
    const choices = typeChoices(entries, { types: new Set(["stock"]) });

    expect(choices.find((choice) => choice.code === "stock")?.count).toBe(0);
  });

  it("count the people and organisations apart, a person staying a person", () => {
    const people = entityOptions(entries, {}, "person", facts);
    const organisations = entityOptions(entries, {}, "organisation", facts);

    expect(people.map((option) => option.entityId).sort()).toStrictEqual([1, 2]);
    expect(organisations.map((option) => option.entityId).sort()).toStrictEqual([10, 11]);
    expect(people.find((o) => o.entityId === 1)?.count).toBe(4);
    expect(organisations.find((o) => o.entityId === 10)?.count).toBe(2);
  });

  it("count a person over what the other filters leave, the persons own choice left out", () => {
    const filter: ExplorerFilter = {
      people: new Set([entityChoice(2)]),
      organisations: new Set([entityChoice(10)]),
    };
    const people = entityOptions(entries, filter, "person", facts);

    // Acme leaves a.pdf and b.pdf: Alice is in both, Bob in neither (he is chosen, so listed).
    expect(people.find((o) => o.entityId === 1)?.count).toBe(2);
    expect(people.find((o) => o.entityId === 2)?.count).toBe(0);
  });

  it("list the roles of a person with the count of files for each", () => {
    const alice = entityOptions(entries, {}, "person", facts).find((o) => o.entityId === 1);

    expect(alice?.roles).toStrictEqual([
      { role: "author", count: 1 },
      { role: "client", count: 1 },
      { role: "mention", count: 1 },
      { role: "subject", count: 1 },
    ]);
  });

  it("keep a chosen name the base no longer knows, with no name", () => {
    const vanished = entityOptions(entries, { people: new Set([entityChoice(99)]) }, "person", facts);

    expect(vanished.find((o) => o.entityId === 99)).toStrictEqual({
      entityId: 99,
      name: null,
      count: 0,
      roles: [],
    });
  });
});

describe("the typed field of names", () => {
  const options = entityOptions(entries, {}, "person", facts);

  it("offers at most five, the most frequent first, and counts the rest", () => {
    const many = Array.from({ length: 8 }, (_, n) => ({
      entityId: 100 + n,
      name: `Name ${n}`,
      count: n,
      roles: [],
    }));
    const { shown, more } = suggestEntities(many, "", new Set());

    expect(shown.map((o) => o.entityId)).toStrictEqual([107, 106, 105, 104, 103]);
    expect(more).toBe(3);
  });

  it("matches inside the name, accents and case ignored", () => {
    const accented = [{ entityId: 1, name: "Hélène Müller", count: 1, roles: [] }];

    expect(suggestEntities(accented, "helene", new Set()).shown).toHaveLength(1);
    expect(suggestEntities(accented, "MULL", new Set()).shown).toHaveLength(1);
    expect(suggestEntities(accented, "zzz", new Set()).shown).toHaveLength(0);
  });

  it("leaves out the names already chosen and the ones the base no longer knows", () => {
    const list = [...options, { entityId: 99, name: null, count: 0, roles: [] }];

    expect(suggestEntities(list, "", new Set([1])).shown.map((o) => o.entityId)).toStrictEqual([2]);
  });

  it("reads the ids of the chosen names whatever their role", () => {
    const ids = chosenEntityIds(new Set([entityChoice(1), entityChoice(2, "author")]));

    expect([...ids]).toStrictEqual([1, 2]);
  });
});

describe("the choices and the pills", () => {
  it("toggle each kind on and off", () => {
    let choices = toggleChoice(NO_CHOICES, "person", entityChoice(1));
    choices = toggleChoice(choices, "organisation", entityChoice(10));
    choices = toggleChoice(choices, "type", "invoice");
    choices = toggleChoice(choices, "year", "2026");

    expect([...choices.people, ...choices.organisations, ...choices.types, ...choices.years]).toStrictEqual([
      "1",
      "10",
      "invoice",
      "2026",
    ]);
    expect(toggleChoice(choices, "person", entityChoice(1)).people.size).toBe(0);
  });

  it("change the role of a name without choosing it twice", () => {
    const any = toggleChoice(NO_CHOICES, "person", entityChoice(1));
    const author = setEntityRole(any, "person", 1, "author");
    const back = setEntityRole(author, "person", 1, null);

    expect([...author.people]).toStrictEqual(["1:author"]);
    expect([...back.people]).toStrictEqual(["1"]);
    expect(author.organisations.size).toBe(0);
  });

  it("show a pill for each kind in a stable order and remove it alone", () => {
    const filter: ExplorerFilter = {
      people: new Set([entityChoice(1)]),
      organisations: new Set([entityChoice(10)]),
      types: new Set(["invoice"]),
      years: new Set(["2026", "2025"]),
    };
    const switches = { recent: true, conversation: true };
    const pills = activePills(filter, [], switches);

    expect(pills.map((pill) => `${pill.kind}:${pill.value}`)).toStrictEqual([
      "person:1",
      "organisation:10",
      "type:invoice",
      "year:2025",
      "year:2026",
      "conversation:conversation",
      "recent:recent",
    ]);
    expect(activeFilterCount(filter, [], switches)).toBe(7);

    const choices = {
      ...NO_CHOICES,
      people: filter.people ?? new Set<string>(),
      years: filter.years ?? new Set<string>(),
      recent: true,
      inConversation: true,
    };
    expect(removePill(choices, pills[0]!).people.size).toBe(0);
    expect([...removePill(choices, pills[4]!).years]).toStrictEqual(["2025"]);
    expect(removePill(choices, pills[5]!).inConversation).toBe(false);
    expect(removePill(choices, pills[6]!).recent).toBe(false);
  });

  it("count the knowledge-base choices and the conversation as filtering, the order alone not", () => {
    expect(isFiltering({ people: new Set([entityChoice(1)]) })).toBe(true);
    expect(isFiltering({ organisations: new Set([entityChoice(1)]) })).toBe(true);
    expect(isFiltering({ types: new Set(["letter"]) })).toBe(true);
    expect(isFiltering({ years: new Set(["2026"]) })).toBe(true);
    expect(isFiltering({ ticked: new Set() })).toBe(true);
    expect(isFiltering({ ticked: null, people: new Set() })).toBe(false);
  });
});
