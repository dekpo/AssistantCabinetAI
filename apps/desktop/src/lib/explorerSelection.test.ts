import { describe, expect, it } from "vitest";
import { noDocumentsScope, scopedPaths, selectionOf, wholeFolderScope } from "./analysisScope";
import { mergeInventories } from "./explorerModel";
import {
  addEntriesToScopes,
  analysedByFamily,
  clearedScopes,
  entriesToTick,
  isFiltering,
  scopeFamilyOf,
  scopeRecord,
  selectionNotes,
  tickableRecords,
  tickedKeysOf,
  usedCount,
  rowClickTicks,
} from "./explorerSelection";
import type { FileRecord, ProcessingStatus } from "./ipc";

function record(relativePath: string, status: ProcessingStatus = "indexed"): FileRecord {
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

const merged = mergeInventories(
  [record("a.pdf"), record("b.pdf"), record("c.pdf", "discovered"), record("grid.xlsx", "indexed")],
  [record("t1.csv"), record("t2.csv", "pending"), record("note.docx", "indexed")],
).entries;

describe("the files a scope is worked out over", () => {
  it("are the analysed files of that family wherever they sit, a foreign one under its key", () => {
    expect(tickableRecords(merged, "documents").map((r) => r.relativePath)).toStrictEqual([
      "a.pdf",
      "b.pdf",
      "/data/note.docx",
    ]);
    expect(tickableRecords(merged, "data").map((r) => r.relativePath)).toStrictEqual([
      "/documents/grid.xlsx",
      "t1.csv",
    ]);
  });

  it("count as every analysed file for 'tous' and as the ticked ones otherwise", () => {
    const documents = tickableRecords(merged, "documents");

    expect(usedCount(noDocumentsScope(1), documents)).toBe(0);
    expect(usedCount(wholeFolderScope(1), documents)).toBe(3);
  });

  it("belong to the scope of their family, not of the folder they sit in", () => {
    const byName = Object.fromEntries(merged.map((e) => [e.name, e]));

    expect(scopeFamilyOf(byName["note.docx"]!)).toBe("documents");
    expect(scopeFamilyOf(byName["grid.xlsx"]!)).toBe("data");
    expect(scopeRecord(byName["note.docx"]!).relativePath).toBe("/data/note.docx");
    expect(scopeRecord(byName["a.pdf"]!).relativePath).toBe("a.pdf");
    // The record keeps the content id the scope pins.
    expect(scopeRecord(byName["grid.xlsx"]!).id).toBe("id-grid.xlsx");
  });

  it("tell two files of one path apart: one in each folder", () => {
    const twins = mergeInventories([record("x.csv"), record("a.pdf")], [record("x.csv")]).entries;

    expect(tickableRecords(twins, "data").map((r) => r.relativePath)).toStrictEqual([
      "/documents/x.csv",
      "x.csv",
    ]);
    const ticked = tickedKeysOf(
      addEntriesToScopes(
        { documents: noDocumentsScope(1), data: noDocumentsScope(1) },
        twins.filter((e) => e.key === "documents:x.csv"),
        twins,
        2,
      ),
      twins,
    );
    expect([...ticked.data]).toStrictEqual(["/documents/x.csv"]);
  });
});

describe("the files 'Cocher les N' adds", () => {
  const none = { documents: noDocumentsScope(1), data: noDocumentsScope(1) };

  it("are the shown files that can be ticked, in both folders, a foreign one once analysed", () => {
    const toTick = entriesToTick(merged, merged, none);

    expect(toTick.map((e) => e.key).sort()).toStrictEqual(
      ["data:note.docx", "data:t1.csv", "documents:a.pdf", "documents:b.pdf", "documents:grid.xlsx"].sort(),
    );
  });

  it("leave out what is already ticked, and everything on a side that is already 'tous'", () => {
    const first = addEntriesToScopes(
      none,
      merged.filter((e) => e.key === "documents:a.pdf"),
      merged,
      5,
    );

    expect(scopedPaths(first.documents)).toStrictEqual(["a.pdf"]);
    expect(entriesToTick(merged, merged, first).map((e) => e.key)).not.toContain("documents:a.pdf");
    expect(
      entriesToTick(merged, merged, { ...first, documents: wholeFolderScope(1) }).map((e) => e.key),
    ).toStrictEqual(["documents:grid.xlsx", "data:t1.csv"]);
  });

  it("only add the shown files: a narrowed view adds only what it shows", () => {
    const shown = merged.filter((e) => e.family === "tables");

    expect(entriesToTick(shown, merged, none).map((e) => e.key)).toStrictEqual([
      "documents:grid.xlsx",
      "data:t1.csv",
    ]);
  });

  it("pin each file added, side by side, with that folder's own rule", () => {
    const next = addEntriesToScopes(none, entriesToTick(merged, merged, none), merged, 9);

    // Every analysed file of the documents side is ticked, so it reads as 'tous', as in the cards.
    expect(selectionOf(next.documents)).toBe("all");
    expect(selectionOf(next.data)).toBe("all");
  });

  it("add a file of the other folder to its own family's scope, under its key", () => {
    const foreign = merged.filter((e) => !e.native && e.state === "analysed");
    const next = addEntriesToScopes(none, foreign, merged, 9);

    expect(scopedPaths(next.documents)).toStrictEqual(["/data/note.docx"]);
    expect(scopedPaths(next.data)).toStrictEqual(["/documents/grid.xlsx"]);
  });

  it("never add a file that is not analysed yet, whichever folder it sits in", () => {
    const waiting = mergeInventories([record("x.xlsx", "discovered")], [record("y.pdf", "pending")])
      .entries;
    const next = addEntriesToScopes(none, waiting, waiting, 9);

    expect(next.documents).toStrictEqual(none.documents);
    expect(next.data).toStrictEqual(none.data);
  });
});

describe("clearing", () => {
  it("empties both sides", () => {
    const cleared = clearedScopes(3);

    expect(selectionOf(cleared.documents)).toBe("none");
    expect(selectionOf(cleared.data)).toBe("none");
  });
});

describe("what narrows the view", () => {
  it("is anything but an empty filter", () => {
    expect(isFiltering({})).toBe(false);
    expect(isFiltering({ family: null, found: null, formats: new Set(), states: new Set() })).toBe(
      false,
    );
    expect(isFiltering({ found: new Set() })).toBe(true);
    expect(isFiltering({ family: "tables" })).toBe(true);
    expect(isFiltering({ formats: new Set(["pdf"]) })).toBe(true);
    expect(isFiltering({ states: new Set(["analysed"]) })).toBe(true);
    expect(isFiltering({ folders: new Set(["documents:clients"]) })).toBe(true);
    expect(isFiltering({ folders: new Set() })).toBe(false);
  });
});

describe("the second line of the bottom bar", () => {
  it("warns once when nothing is used on either side", () => {
    expect(selectionNotes("none", "none")).toStrictEqual(["none"]);
  });

  it("warns about each whole family used", () => {
    expect(selectionNotes("all", "some")).toStrictEqual(["wholeDocuments"]);
    expect(selectionNotes("none", "all")).toStrictEqual(["wholeTables"]);
    expect(selectionNotes("all", "all")).toStrictEqual(["wholeDocuments", "wholeTables"]);
  });

  it("says nothing for a chosen list", () => {
    expect(selectionNotes("some", "none")).toStrictEqual([]);
    expect(selectionNotes("some", "some")).toStrictEqual([]);
  });
});

describe("a click on a file's row", () => {
  it("ticks a tickable row wherever it lands, except on a control of the row", () => {
    expect(rowClickTicks({ onControl: false, tickable: true })).toBe(true);
    expect(rowClickTicks({ onControl: true, tickable: true })).toBe(false);
  });

  it("ticks nothing while the row cannot be ticked", () => {
    expect(rowClickTicks({ onControl: false, tickable: false })).toBe(false);
    expect(rowClickTicks({ onControl: true, tickable: false })).toBe(false);
  });
});

describe("the analysed files of each family, from the two folders' inventories", () => {
  it("are the files the index knows by key: a file of the other folder under its folder's name", () => {
    const found = analysedByFamily(
      [record("a.pdf"), record("grid.xlsx"), record("c.pdf", "discovered")],
      [record("t1.csv"), record("note.docx"), record("noext", "indexed")],
    );

    expect(found.documents.map((r) => r.relativePath)).toStrictEqual(["a.pdf", "/data/note.docx"]);
    expect(found.data.map((r) => r.relativePath)).toStrictEqual(["/documents/grid.xlsx", "t1.csv"]);
  });
});
