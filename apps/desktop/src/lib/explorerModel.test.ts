import { describe, expect, it } from "vitest";
import {
  countsByFamily,
  DOCUMENT_EXTENSIONS,
  duplicateNames,
  explorerKey,
  folderKey,
  listSort,
  familyOf,
  filterEntries,
  folderHint,
  foundKeys,
  isTickable,
  mergeInventories,
  sortEntries,
  stateOf,
  TABLE_EXTENSIONS,
  type ExplorerEntry,
} from "./explorerModel";
import type { FileRecord, ProcessingStatus } from "./ipc";

function record(
  relativePath: string,
  status: ProcessingStatus = "indexed",
  modifiedAt: number | null = 1000,
): FileRecord {
  const name = relativePath.split("/").pop() ?? relativePath;
  const dot = name.lastIndexOf(".");
  return {
    id: `id-${relativePath}`,
    relativePath,
    name,
    stem: dot < 0 ? name : name.slice(0, dot),
    extension: dot < 0 ? "" : name.slice(dot + 1).toLowerCase(),
    sizeBytes: 10,
    modifiedAt,
    processingStatus: status,
  } as FileRecord;
}

function entries(documents: FileRecord[], data: FileRecord[]): ExplorerEntry[] {
  return mergeInventories(documents, data).entries;
}

describe("the family of an extension", () => {
  it.each([...DOCUMENT_EXTENSIONS])("%s is a document", (extension) => {
    expect(familyOf(extension)).toBe("documents");
  });

  it.each([...TABLE_EXTENSIONS])("%s is a table", (extension) => {
    expect(familyOf(extension)).toBe("tables");
  });

  it("reads the dot and the case, and keeps unknown formats apart", () => {
    expect(familyOf(".PDF")).toBe("documents");
    expect(familyOf(" XLSX ")).toBe("tables");
    expect(familyOf("pages")).toBeNull();
    expect(familyOf("")).toBeNull();
  });

  it("never lists one extension in both families", () => {
    expect(DOCUMENT_EXTENSIONS.filter((e) => TABLE_EXTENSIONS.includes(e))).toStrictEqual([]);
  });
});

describe("the key of a file", () => {
  it("is never the path alone: one relative path in two folders is two files", () => {
    expect(explorerKey("documents", "a/b.pdf")).not.toBe(explorerKey("data", "a/b.pdf"));
  });
});

describe("the state of a file", () => {
  it("maps the inventory's status to the three states of the screen", () => {
    expect(stateOf(record("a.pdf", "indexed"), true)).toBe("analysed");
    expect(stateOf(record("a.pdf", "failed"), true)).toBe("unreadable");
    for (const status of ["discovered", "pending", "processing"] as const) {
      expect(stateOf(record("a.pdf", status), true)).toBe("toAnalyse");
    }
  });

  it("is always 'to analyse' for a file of the other family, whatever its own folder says", () => {
    // A document in the Data folder is forced to failed there; a workbook in the Documents folder
    // is not assessed: neither says anything about a pipeline that is not theirs.
    expect(stateOf(record("a.docx", "failed"), false)).toBe("toAnalyse");
    expect(stateOf(record("a.xlsx", "discovered"), false)).toBe("toAnalyse");
    expect(stateOf(record("a.xlsx", "indexed"), false)).toBe("toAnalyse");
  });
});

describe("merging the two inventories", () => {
  it("lists a workbook of the Documents folder under Tables, tagged as foreign", () => {
    const [entry] = entries([record("planning.xlsx", "discovered")], []);

    expect(entry?.family).toBe("tables");
    expect(entry?.root).toBe("documents");
    expect(entry?.native).toBe(false);
    expect(entry?.state).toBe("toAnalyse");
  });

  it("lists a document of the Data folder under Documents, as 'to analyse' not 'unreadable'", () => {
    const [entry] = entries([], [record("lettre.docx", "failed")]);

    expect(entry?.family).toBe("documents");
    expect(entry?.root).toBe("data");
    expect(entry?.native).toBe(false);
    expect(entry?.state).toBe("toAnalyse");
  });

  it("keeps a file of its own family native", () => {
    const merged = entries([record("a.pdf")], [record("b.csv", "failed")]);

    expect(merged.map((e) => [e.key, e.native, e.state])).toStrictEqual([
      ["documents:a.pdf", true, "analysed"],
      ["data:b.csv", true, "unreadable"],
    ]);
  });

  it("sets aside the files no pipeline reads, and counts them", () => {
    const merged = mergeInventories(
      [record("a.pdf"), record("keynote.key"), record("noext")],
      [record("old.pages")],
    );

    expect(merged.entries).toHaveLength(1);
    expect(merged.unsupported).toBe(3);
  });

  it("keeps one relative path found in both folders as two files when the folders are separate", () => {
    const merged = mergeInventories([record("notes/a.pdf")], [record("notes/a.pdf")], {
      documents: "/home/me/Docs",
      data: "/home/me/Data",
    });

    expect(merged.entries.map((e) => e.key)).toStrictEqual([
      "documents:notes/a.pdf",
      "data:notes/a.pdf",
    ]);
  });

  it("keeps one entry for a file both folders list when the folders overlap, the native copy winning", () => {
    // The Data folder sits inside the Documents folder: the workbook is listed twice.
    const merged = mergeInventories(
      [record("Data/sub/ventes.csv", "discovered"), record("lettre.pdf")],
      [record("sub/ventes.csv", "indexed")],
      { documents: "C:\\Users\\me\\Docs\\", data: "c:/users/me/docs/data" },
    );

    expect(merged.entries.map((e) => e.key)).toStrictEqual([
      "data:sub/ventes.csv",
      "documents:lettre.pdf",
    ]);
    expect(merged.entries[0]?.state).toBe("analysed");
  });

  it("does not merge anything without the folders' paths", () => {
    const merged = mergeInventories([record("a.csv")], [record("a.csv")]);

    expect(merged.entries).toHaveLength(2);
  });

  it("keeps the parent folder of a file", () => {
    const [nested, top] = entries([record("a/b/c.pdf"), record("d.pdf")], []);

    expect(nested?.folder).toBe("a/b");
    expect(top?.folder).toBe("");
  });
});

describe("filtering", () => {
  const all = entries(
    [
      record("a.pdf", "indexed"),
      record("b.docx", "discovered"),
      record("c.pdf", "failed"),
      record("grid.xlsx", "discovered"),
    ],
    [record("t1.csv", "indexed"), record("t2.xlsx", "pending"), record("note.docx", "failed")],
  );
  const names = (list: ExplorerEntry[]) => list.map((e) => e.name);

  it("keeps everything for an empty filter", () => {
    expect(filterEntries(all, {})).toHaveLength(all.length);
    expect(filterEntries(all, { family: null, formats: new Set(), states: new Set(), found: null }))
      .toHaveLength(all.length);
  });

  it("filters by family, wherever the file sits", () => {
    expect(names(filterEntries(all, { family: "tables" })).sort()).toStrictEqual(
      ["grid.xlsx", "t1.csv", "t2.xlsx"].sort(),
    );
    expect(names(filterEntries(all, { family: "documents" }))).toContain("note.docx");
  });

  it("filters by format and by state", () => {
    expect(names(filterEntries(all, { formats: new Set(["pdf"]) }))).toStrictEqual([
      "a.pdf",
      "c.pdf",
    ]);
    expect(names(filterEntries(all, { states: new Set(["unreadable"]) }))).toStrictEqual(["c.pdf"]);
  });

  it("filters by the keys a search found", () => {
    const found = new Set([explorerKey("documents", "a.pdf"), explorerKey("data", "t1.csv")]);

    expect(names(filterEntries(all, { found }))).toStrictEqual(["a.pdf", "t1.csv"]);
    expect(filterEntries(all, { found: new Set() })).toStrictEqual([]);
  });

  it("combines every filter", () => {
    const result = filterEntries(all, {
      family: "documents",
      formats: new Set(["pdf", "docx"]),
      states: new Set(["toAnalyse"]),
    });

    // b.docx (Documents folder, pending) and note.docx (foreign, always to analyse).
    expect(names(result).sort()).toStrictEqual(["b.docx", "note.docx"]);
  });

  it("joins the two folders' search answers by (folder, path)", () => {
    expect(foundKeys(null, null)).toBeNull();
    expect([...(foundKeys(["a.pdf"], null) ?? [])]).toStrictEqual(["documents:a.pdf"]);
    expect([...(foundKeys(["x.csv"], ["x.csv"]) ?? [])].sort()).toStrictEqual([
      "data:x.csv",
      "documents:x.csv",
    ]);
  });
});

describe("sorting", () => {
  it("sorts by name with numbers read as numbers and accents ignored", () => {
    const sorted = sortEntries(
      entries([record("note10.pdf"), record("note2.pdf"), record("Zoe.pdf"), record("\u00e9t\u00e9.pdf")], []),
      "name",
    );

    expect(sorted.map((e) => e.name)).toStrictEqual([
      "\u00e9t\u00e9.pdf",
      "note2.pdf",
      "note10.pdf",
      "Zoe.pdf",
    ]);
  });

  it("sorts by date, newest first, undated files last", () => {
    const sorted = sortEntries(
      entries([record("old.pdf", "indexed", 100), record("none.pdf", "indexed", null), record("new.pdf", "indexed", 900)], []),
      "date",
    );

    expect(sorted.map((e) => e.name)).toStrictEqual(["new.pdf", "old.pdf", "none.pdf"]);
  });

  it("sorts by state: what asks for something first, analysed last", () => {
    const sorted = sortEntries(
      entries(
        [record("a.pdf", "indexed"), record("b.pdf", "failed"), record("c.pdf", "pending")],
        [],
      ),
      "state",
    );

    expect(sorted.map((e) => e.name)).toStrictEqual(["c.pdf", "b.pdf", "a.pdf"]);
  });

  it("does not change the list it is given", () => {
    const list = entries([record("b.pdf"), record("a.pdf")], []);

    sortEntries(list, "name");

    expect(list.map((e) => e.name)).toStrictEqual(["b.pdf", "a.pdf"]);
  });
});

describe("counts", () => {
  const all = entries(
    [
      record("a.pdf", "indexed"),
      record("b.pdf", "discovered"),
      record("c.pdf", "failed"),
      record("grid.xlsx", "discovered"),
    ],
    [record("t1.csv", "indexed"), record("note.docx", "failed"), record("t2.csv", "pending")],
  );

  it("counts each family by state", () => {
    const counts = countsByFamily(all);

    expect(counts.documents).toMatchObject({ total: 4, analysed: 1, toAnalyse: 2, unreadable: 1 });
    expect(counts.tables).toMatchObject({ total: 3, analysed: 1, toAnalyse: 2, unreadable: 0 });
  });

  it("leaves the foreign files out of what Analyser counts, and says how many there are", () => {
    const counts = countsByFamily(all);

    // Documents: b.pdf only (note.docx is a document sitting in the Data folder).
    expect(counts.documents.analysable).toBe(1);
    expect(counts.documents.foreign).toBe(1);
    // Tables: t2.csv only (grid.xlsx sits in the Documents folder).
    expect(counts.tables.analysable).toBe(1);
    expect(counts.tables.foreign).toBe(1);
  });

  it("only lets an analysed file of its own folder be ticked", () => {
    const byName = Object.fromEntries(all.map((e) => [e.name, e]));

    expect(isTickable(byName["a.pdf"] as ExplorerEntry)).toBe(true);
    expect(isTickable(byName["b.pdf"] as ExplorerEntry)).toBe(false);
    expect(isTickable(byName["grid.xlsx"] as ExplorerEntry)).toBe(false);
  });
});

describe("two files that share a name", () => {
  const all = entries(
    [record("patients/dupont/note.docx"), record("admin/note.docx"), record("solo.pdf")],
    [],
  );

  it("finds the shared names, ignoring case", () => {
    const names = duplicateNames([...all, ...entries([record("Admin/NOTE.docx")], [])]);

    expect([...names]).toStrictEqual(["note.docx"]);
    expect(duplicateNames(entries([record("solo.pdf")], [])).size).toBe(0);
  });

  it("hints with the last folder of the path, and nothing for a file at the top", () => {
    expect(folderHint(all[0] as ExplorerEntry)).toBe("dupont");
    expect(folderHint(all[1] as ExplorerEntry)).toBe("admin");
    expect(folderHint(all[2] as ExplorerEntry)).toBe("");
  });
});

describe("the order a list takes", () => {
  const rows = entries(
    [record("b.pdf"), record("a.pdf"), record("c.pdf"), record("Notes/2026-10-20_18-51-28.md")],
    [],
  );

  it("follows the finder's ranking while a search is active, the unranked last by name", () => {
    const rank = new Map([
      [explorerKey("documents", "Notes/2026-10-20_18-51-28.md"), 0],
      [explorerKey("documents", "c.pdf"), 1],
    ]);

    expect(sortEntries(rows, "relevance", rank).map((entry) => entry.path)).toStrictEqual([
      "Notes/2026-10-20_18-51-28.md",
      "c.pdf",
      "a.pdf",
      "b.pdf",
    ]);
  });

  it("is relevance while searching, the newest first for the Notes folder alone, else by name", () => {
    const notes = folderKey("documents", "Notes");

    expect(listSort({ searching: true, folders: new Set(), notesKey: notes })).toBe("relevance");
    expect(listSort({ searching: false, folders: new Set([notes]), notesKey: notes })).toBe("date");
    expect(listSort({ searching: false, folders: new Set(), notesKey: notes })).toBe("name");
    expect(
      listSort({ searching: false, folders: new Set([notes, "documents:2025"]), notesKey: notes }),
    ).toBe("name");
    expect(listSort({ searching: false, folders: new Set([notes]), notesKey: null })).toBe("name");
  });

  it("puts the newest first whenever the Recents order was asked for, even while searching", () => {
    expect(listSort({ searching: false, folders: new Set(), notesKey: null, recent: true })).toBe("date");
    expect(listSort({ searching: true, folders: new Set(), notesKey: null, recent: true })).toBe("date");
    expect(listSort({ searching: true, folders: new Set(), notesKey: null, recent: false })).toBe(
      "relevance",
    );
  });

  it("counts the Notes folder and what is under it, not a folder of the same name elsewhere", () => {
    const all = entries(
      [
        record("Notes/2026-10-20_18-51-28.md"),
        record("Notes/old/2026-01-01_a.md"),
        record("administratif/notes/2026-02-10_note-interne.docx"),
        record("a.pdf"),
      ],
      [],
    );

    expect(
      filterEntries(all, { folders: new Set([folderKey("documents", "Notes")]) }).map((e) => e.path),
    ).toStrictEqual(["Notes/2026-10-20_18-51-28.md", "Notes/old/2026-01-01_a.md"]);
  });
});
