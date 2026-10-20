import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { mergeInventories } from "./explorerModel";
import type { FileRecord, ProcessingStatus } from "./ipc";
import { familiesToUnfold, foldsToRestore, NO_VIEW, viewShowingState } from "./explorerView";

function record(relativePath: string, status: ProcessingStatus): FileRecord {
  const name = relativePath.split("/").pop() ?? relativePath;
  const dot = name.lastIndexOf(".");
  return {
    id: relativePath,
    relativePath,
    name,
    stem: name.slice(0, dot),
    extension: name.slice(dot + 1),
    sizeBytes: 1,
    modifiedAt: 1,
    processingStatus: status,
  } as FileRecord;
}

describe("the view an unreadable-files link sets", () => {
  it("replaces every other choice and keeps nothing of the typed text (the caller empties it)", () => {
    const view = viewShowingState("unreadable");

    expect(view).not.toBe(NO_VIEW);
    expect([...view.choices.states]).toStrictEqual(["unreadable"]);
    expect(view.family).toBe("all");
  });
});

describe("the lists the link unfolds", () => {
  const entries = mergeInventories(
    [record("a.pdf", "failed"), record("b.pdf", "indexed")],
    [record("t.csv", "indexed")],
  ).entries;
  const bothFolded = { documents: true, tables: true };

  it("opens only a folded list that holds some of the files", () => {
    expect(familiesToUnfold(entries, "unreadable", bothFolded)).toStrictEqual(["documents"]);
  });

  it("leaves an open list and a list with none of them alone", () => {
    expect(familiesToUnfold(entries, "unreadable", { documents: false, tables: true })).toStrictEqual([]);
    expect(familiesToUnfold([], "unreadable", bothFolded)).toStrictEqual([]);
  });

  it("opens nothing for another state it was not asked about", () => {
    expect(familiesToUnfold(entries, "toAnalyse", bothFolded)).toStrictEqual([]);
  });
});

describe("a filter never folds or unfolds a list", () => {
  it("leaves the fold to the arrow button and to the overview's link, nothing else", () => {
    const source = readFileSync(new URL("../components/FileExplorer.tsx", import.meta.url), "utf8");
    // Two calls, one per family's arrow button: no pill, filter, search or pass touches a fold.
    expect(source.match(/onFoldedChange\(/g)).toHaveLength(2);
    expect(Object.keys(NO_VIEW).sort()).toStrictEqual(["choices", "family"]);
  });
});

describe("the lists the link opened close when its filter is removed", () => {
  it("closes them once the unreadable choice is gone, and only then", () => {
    const withFilter = viewShowingState("unreadable");

    expect(foldsToRestore(["documents"], withFilter)).toStrictEqual([]);
    expect(foldsToRestore(["documents"], NO_VIEW)).toStrictEqual(["documents"]);
    expect(foldsToRestore([], NO_VIEW)).toStrictEqual([]);
  });

  it("leaves them alone while another choice changes and the unreadable one stays", () => {
    const view = viewShowingState("unreadable", "documents");

    expect(foldsToRestore(["documents", "tables"], view)).toStrictEqual([]);
  });
});
