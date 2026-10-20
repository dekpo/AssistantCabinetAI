import { describe, expect, it } from "vitest";
import { noDocumentsScope, scopedPaths, selectionOf, wholeFolderScope } from "./analysisScope";
import { mergeInventories } from "./explorerModel";
import {
  addEntriesToScopes,
  clearedScopes,
  entriesToTick,
  isFiltering,
  selectionNotes,
  tickableRecords,
  usedCount,
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
  it("are the analysed files of that folder's own family, never a foreign one", () => {
    expect(tickableRecords(merged, "documents").map((r) => r.relativePath)).toStrictEqual([
      "a.pdf",
      "b.pdf",
    ]);
    expect(tickableRecords(merged, "data").map((r) => r.relativePath)).toStrictEqual(["t1.csv"]);
  });

  it("count as every analysed file for 'tous' and as the ticked ones otherwise", () => {
    const documents = tickableRecords(merged, "documents");

    expect(usedCount(noDocumentsScope(1), documents)).toBe(0);
    expect(usedCount(wholeFolderScope(1), documents)).toBe(2);
  });
});

describe("the files 'Cocher les N' adds", () => {
  const none = { documents: noDocumentsScope(1), data: noDocumentsScope(1) };

  it("are the shown files that can be ticked, in both folders, and not a foreign one", () => {
    const toTick = entriesToTick(merged, merged, none);

    expect(toTick.map((e) => e.key).sort()).toStrictEqual(
      ["data:t1.csv", "documents:a.pdf", "documents:b.pdf"].sort(),
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
    ).toStrictEqual(["data:t1.csv"]);
  });

  it("only add the shown files: a narrowed view adds only what it shows", () => {
    const shown = merged.filter((e) => e.family === "tables");

    expect(entriesToTick(shown, merged, none).map((e) => e.key)).toStrictEqual(["data:t1.csv"]);
  });

  it("pin each file added, side by side, with that folder's own rule", () => {
    const next = addEntriesToScopes(none, entriesToTick(merged, merged, none), merged, 9);

    // Every analysed file of the documents side is ticked, so it reads as 'tous', as in the cards.
    expect(selectionOf(next.documents)).toBe("all");
    expect(selectionOf(next.data)).toBe("all");
  });

  it("never add a file of the other family", () => {
    const foreign = merged.filter((e) => !e.native);
    const next = addEntriesToScopes(none, foreign, merged, 9);

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
