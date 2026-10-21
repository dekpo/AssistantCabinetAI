import { describe, expect, it } from "vitest";
import {
  countsByFamily,
  DOCUMENT_EXTENSIONS,
  duplicateNames,
  entryIndexKey,
  explorerKey,
  explorerKeyOfIndexKey,
  folderKey,
  listSort,
  familyOf,
  filterEntries,
  folderHint,
  foundKeys,
  isOpenable,
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
    expect(stateOf(record("a.pdf", "indexed"))).toBe("analysed");
    expect(stateOf(record("a.pdf", "failed"))).toBe("unreadable");
    for (const status of ["discovered", "pending", "processing"] as const) {
      expect(stateOf(record("a.pdf", status))).toBe("toAnalyse");
    }
  });

  it("reads the folder's verdict for a file of the other family: its own family has analysed it there", () => {
    // Each inventory joins a file to what its own family made of it, wherever it sits: a workbook of
    // the Documents folder is green once the tabular pass has read it, a document of the Data folder
    // once the document pass has.
    expect(stateOf(record("a.docx", "failed"))).toBe("unreadable");
    expect(stateOf(record("a.xlsx", "discovered"))).toBe("toAnalyse");
    expect(stateOf(record("a.xlsx", "indexed"))).toBe("analysed");
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

  it("lists a document of the Data folder under Documents, tagged as foreign, in its own state", () => {
    const [waiting] = entries([], [record("lettre.docx", "discovered")]);
    const [read] = entries([], [record("lettre.docx", "indexed")]);

    expect(waiting?.family).toBe("documents");
    expect(waiting?.root).toBe("data");
    expect(waiting?.native).toBe(false);
    expect(waiting?.state).toBe("toAnalyse");
    expect(read?.state).toBe("analysed");
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

  describe("any two folders, never a duplicate", () => {
    // One document and one workbook at the top, one of each in a sub-folder, and a file no pipeline reads.
    const keysOf = (merged: ReturnType<typeof mergeInventories>) =>
      merged.entries.map((e) => `${e.key}|${entryIndexKey(e)}|${e.native ? "native" : "foreign"}`).sort();

    it("the same folder: one entry per physical file, every file native, no foreign key", () => {
      const listing = () => [
        record("lettre.pdf"),
        record("suivi.csv"),
        record("sub/note.txt"),
        record("sub/stock.xlsx"),
        record("sub/readme.key"),
      ];
      const merged = mergeInventories(listing(), listing(), {
        documents: "/w/Both",
        data: "/w/Both",
      });

      expect(keysOf(merged)).toStrictEqual([
        "data:sub/stock.xlsx|sub/stock.xlsx|native",
        "data:suivi.csv|suivi.csv|native",
        "documents:lettre.pdf|lettre.pdf|native",
        "documents:sub/note.txt|sub/note.txt|native",
      ]);
      expect(merged.unsupported).toBe(1);
      expect(merged.entries.some((e) => entryIndexKey(e).startsWith("/"))).toBe(false);
    });

    it("tables inside documents: the nested folder's own files native, the rest of the folder foreign", () => {
      const merged = mergeInventories(
        // The Documents folder lists everything under it.
        [
          record("lettre.pdf"),
          record("b.csv"),
          record("Tables/a.csv"),
          record("Tables/c.pdf"),
        ],
        // The Tables folder lists what is inside it.
        [record("a.csv"), record("c.pdf")],
        { documents: "/w/Docs", data: "/w/Docs/Tables" },
      );

      expect(keysOf(merged)).toStrictEqual([
        "data:a.csv|a.csv|native",
        "documents:Tables/c.pdf|Tables/c.pdf|native",
        "documents:b.csv|/documents/b.csv|foreign",
        "documents:lettre.pdf|lettre.pdf|native",
      ]);
    });

    it("documents inside tables: the same, the other way round", () => {
      const merged = mergeInventories(
        [record("lettre.pdf"), record("t.csv")],
        [record("Docs/lettre.pdf"), record("Docs/t.csv"), record("d.pdf"), record("u.csv")],
        { documents: "/w/Data/Docs", data: "/w/Data" },
      );

      expect(keysOf(merged)).toStrictEqual([
        "data:Docs/t.csv|Docs/t.csv|native",
        "data:d.pdf|/data/d.pdf|foreign",
        "data:u.csv|u.csv|native",
        "documents:lettre.pdf|lettre.pdf|native",
      ]);
    });

    it("an analysed file of any layout is opened from the root and the path of its one entry, and a key leads to the same", () => {
      const layouts = [
        // Tables inside documents.
        mergeInventories(
          [record("b.csv"), record("Tables/a.csv"), record("Tables/c.pdf")],
          [record("a.csv"), record("c.pdf")],
          { documents: "/w/Docs", data: "/w/Docs/Tables" },
        ),
        // Documents inside tables (covered by automated tests only).
        mergeInventories(
          [record("lettre.pdf"), record("t.csv")],
          [record("Docs/lettre.pdf"), record("Docs/t.csv"), record("d.pdf"), record("u.csv")],
          { documents: "/w/Data/Docs", data: "/w/Data" },
        ),
      ];

      for (const merged of layouts) {
        const tabs = new Set<string>();
        for (const entry of merged.entries) {
          expect(isOpenable(entry)).toBe(true);
          // What Ouvrir hands to the viewer: the root and the plain path of the entry itself.
          tabs.add(`${entry.root}\u0000${entry.path}`);
          // And the key the index knows it by points back at that very entry.
          const family = entry.family === "documents" ? "documents" : "data";
          expect(explorerKeyOfIndexKey(family, entryIndexKey(entry))).toBe(entry.key);
        }
        // One tab per physical file: no entry is listed twice.
        expect(tabs.size).toBe(merged.entries.length);
      }
    });

    it("every key of those layouts leads back to the one entry that holds the file", () => {
      const merged = mergeInventories(
        [record("b.csv"), record("Tables/a.csv"), record("lettre.pdf")],
        [record("a.csv")],
        { documents: "/w/Docs", data: "/w/Docs/Tables" },
      );
      const byKey = new Map(merged.entries.map((e) => [e.key, e]));

      expect(byKey.get(explorerKeyOfIndexKey("data", "a.csv"))?.path).toBe("a.csv");
      expect(byKey.get(explorerKeyOfIndexKey("data", "/documents/b.csv"))?.path).toBe("b.csv");
      expect(byKey.get(explorerKeyOfIndexKey("documents", "lettre.pdf"))?.path).toBe("lettre.pdf");
    });

    it("counts a file no pipeline reads once, however many folders list it", () => {
      const merged = mergeInventories([record("sub/a.key")], [record("a.key")], {
        documents: "/w/Docs",
        data: "/w/Docs/sub",
      });

      expect(merged.unsupported).toBe(1);
    });
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
    [record("t1.csv", "indexed"), record("t2.xlsx", "pending"), record("note.docx", "discovered")],
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

    // b.docx (Documents folder, pending) and note.docx (foreign, not analysed yet).
    expect(names(result).sort()).toStrictEqual(["b.docx", "note.docx"]);
  });

  it("joins the two families' search answers by (folder, path), the folder being the one the key names", () => {
    expect(foundKeys(null, null)).toBeNull();
    expect([...(foundKeys(["a.pdf"], null) ?? [])]).toStrictEqual(["documents:a.pdf"]);
    expect([...(foundKeys(["x.csv"], ["x.csv"]) ?? [])].sort()).toStrictEqual([
      "data:x.csv",
      "documents:x.csv",
    ]);
    // A workbook of the Documents folder and a document of the Data folder come back as keys.
    expect(
      [...(foundKeys(["/data/lettre.pdf", "a.pdf"], ["/documents/x.csv", "x.csv"]) ?? [])].sort(),
    ).toStrictEqual(["data:lettre.pdf", "data:x.csv", "documents:a.pdf", "documents:x.csv"]);
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
    [record("t1.csv", "indexed"), record("note.docx", "discovered"), record("t2.csv", "pending")],
  );

  it("counts each family by state", () => {
    const counts = countsByFamily(all);

    expect(counts.documents).toMatchObject({ total: 4, analysed: 1, toAnalyse: 2, unreadable: 1 });
    expect(counts.tables).toMatchObject({ total: 3, analysed: 1, toAnalyse: 2, unreadable: 0 });
  });

  it("counts the foreign files in what Analyser does, and still says how many there are", () => {
    const counts = countsByFamily(all);

    // Documents: b.pdf and note.docx (a document sitting in the Data folder, read there).
    expect(counts.documents.analysable).toBe(2);
    expect(counts.documents.foreign).toBe(1);
    // Tables: t2.csv and grid.xlsx (a workbook sitting in the Documents folder, read there).
    expect(counts.tables.analysable).toBe(2);
    expect(counts.tables.foreign).toBe(1);
  });

  it("lets an analysed file be ticked and opened wherever it sits", () => {
    const withForeign = entries(
      [record("a.pdf", "indexed"), record("b.pdf", "discovered"), record("grid.xlsx", "indexed")],
      [record("note.docx", "indexed")],
    );
    const byName = Object.fromEntries(withForeign.map((e) => [e.name, e]));

    expect(isTickable(byName["a.pdf"] as ExplorerEntry)).toBe(true);
    expect(isTickable(byName["b.pdf"] as ExplorerEntry)).toBe(false);
    expect(isTickable(byName["grid.xlsx"] as ExplorerEntry)).toBe(true);
    expect(isTickable(byName["note.docx"] as ExplorerEntry)).toBe(true);
    expect(isOpenable(byName["a.pdf"] as ExplorerEntry)).toBe(true);
    expect(isOpenable(byName["b.pdf"] as ExplorerEntry)).toBe(false);
    expect(isOpenable(byName["grid.xlsx"] as ExplorerEntry)).toBe(true);
    expect(isOpenable(byName["note.docx"] as ExplorerEntry)).toBe(true);
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
