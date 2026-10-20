import { describe, expect, it } from "vitest";
import {
  activeFilterCount,
  activePills,
  folderChoices,
  folderOfKey,
  formatChoices,
  leftByOthers,
  MAX_FOLDER_SUGGESTIONS,
  NO_CHOICES,
  reasonKeys,
  removePill,
  stateCounts,
  suggestFolders,
  toggleChoice,
  withoutTypedFormat,
  withoutTypedFormats,
} from "./explorerFilters";
import {
  filterEntries,
  folderKey,
  foundKeys,
  mergeInventories,
  type ExplorerEntry,
  type ExplorerFilter,
} from "./explorerModel";
import { addEntriesToScopes, entriesToTick } from "./explorerSelection";
import { noDocumentsScope, selectionOf } from "./analysisScope";
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

/* Documents folder: three pdf (one not analysed), two docx, one at the top. Data folder: two csv, one
   xlsx, and a sub-folder `2025` that the Documents folder holds too. */
const entries = mergeInventories(
  [
    record("top.pdf"),
    record("clients/2025/a.pdf"),
    record("clients/2025/b.pdf", "discovered"),
    record("clients/2026/c.docx"),
    record("clients/2026/d.docx", "failed"),
    record("2025/old.pdf"),
  ],
  [record("2025/t1.csv"), record("2025/t2.csv"), record("budget/grid.xlsx")],
).entries;

const keyOf = (e: ExplorerEntry) => e.key;
const none: ExplorerFilter = {};

describe("the origin-folder filter", () => {
  it("keeps the files in the folder and under it, in that root only", () => {
    const filter = { folders: new Set([folderKey("documents", "clients")]) };

    expect(filterEntries(entries, filter).map(keyOf)).toStrictEqual([
      "documents:clients/2025/a.pdf",
      "documents:clients/2025/b.pdf",
      "documents:clients/2026/c.docx",
      "documents:clients/2026/d.docx",
    ]);
  });

  it("tells the two roots apart when both hold a folder of the same name", () => {
    const docs = { folders: new Set([folderKey("documents", "2025")]) };
    const data = { folders: new Set([folderKey("data", "2025")]) };

    expect(filterEntries(entries, docs).map(keyOf)).toStrictEqual(["documents:2025/old.pdf"]);
    expect(filterEntries(entries, data).map(keyOf)).toStrictEqual([
      "data:2025/t1.csv",
      "data:2025/t2.csv",
    ]);
  });

  it("does not take a folder whose name only starts the same", () => {
    const filter = { folders: new Set([folderKey("documents", "clients/202")]) };

    expect(filterEntries(entries, filter)).toStrictEqual([]);
  });

  it("combines with every other filter: the kinds are an and, the choices of a kind an or", () => {
    const filter: ExplorerFilter = {
      folders: new Set([folderKey("documents", "clients")]),
      formats: new Set(["pdf", "docx"]),
      states: new Set(["analysed"]),
    };

    expect(filterEntries(entries, filter).map(keyOf)).toStrictEqual([
      "documents:clients/2025/a.pdf",
      "documents:clients/2026/c.docx",
    ]);
  });
});

describe("the number beside a choice", () => {
  it("is counted over what the other filters leave, with its own kind left out", () => {
    const filter: ExplorerFilter = {
      formats: new Set(["pdf"]),
      states: new Set(["analysed"]),
    };

    // Formats ignore the format choice but obey the state: analysed files only.
    expect(formatChoices(entries, filter, [])).toStrictEqual([
      { extension: "csv", count: 2 },
      { extension: "pdf", count: 3 },
      { extension: "docx", count: 1 },
      { extension: "xlsx", count: 1 },
    ].sort((a, b) => b.count - a.count || a.extension.localeCompare(b.extension)));
    // States ignore the state choice but obey the format: the pdf files only.
    expect(stateCounts(entries, filter)).toStrictEqual({ analysed: 3, toAnalyse: 1, unreadable: 0 });
  });

  it("says 0 for a choice that would empty the list, and still lists the format", () => {
    const filter: ExplorerFilter = { folders: new Set([folderKey("data", "budget")]) };
    const choices = formatChoices(entries, filter, []);

    expect(choices.find((c) => c.extension === "pdf")).toStrictEqual({ extension: "pdf", count: 0 });
    expect(choices.find((c) => c.extension === "xlsx")).toStrictEqual({ extension: "xlsx", count: 1 });
  });

  it("lists only the formats present, and a ticked or typed one even with no file left", () => {
    expect(formatChoices(entries, none, []).map((c) => c.extension).sort()).toStrictEqual([
      "csv",
      "docx",
      "pdf",
      "xlsx",
    ]);
    expect(formatChoices(entries, { formats: new Set(["odt"]) }, ["rtf"]).map((c) => c.extension)).toEqual(
      expect.arrayContaining(["odt", "rtf"]),
    );
  });

  it("counts the families over the other filters, so the segmented control never lies", () => {
    const filter: ExplorerFilter = { family: "tables", states: new Set(["analysed"]) };
    const forSegments = leftByOthers(entries, filter, "family");

    // Four documents are analysed; the tables are counted too, though the family filter is on them.
    expect(forSegments.filter((e) => e.family === "documents")).toHaveLength(4);
    expect(forSegments.filter((e) => e.family === "tables")).toHaveLength(3);
  });

  it("counts the families over the search results too", () => {
    const filter: ExplorerFilter = { found: foundKeys(["top.pdf", "clients/2026/c.docx"], []) };

    expect(leftByOthers(entries, filter, "family")).toHaveLength(2);
  });
});

describe("a typed extension beside a format choice", () => {
  it("narrows: the search found the docx files, so a pdf choice shows 0 before it is ticked", () => {
    // The finder took `.docx` out of the text and answered the docx files.
    const found = foundKeys(["clients/2026/c.docx", "clients/2026/d.docx"], []);
    const filter: ExplorerFilter = { found };
    const choices = formatChoices(entries, filter, ["docx"]);

    expect(choices.find((c) => c.extension === "pdf")?.count).toBe(0);
    expect(choices.find((c) => c.extension === "docx")?.count).toBe(2);
    expect(filterEntries(entries, { ...filter, formats: new Set(["pdf"]) })).toStrictEqual([]);
  });
});

describe("the origin-folder suggestions", () => {
  const choices = folderChoices(entries, none);

  it("offer every folder and each of its ancestors, counted over the files in it or under it", () => {
    const count = (root: "documents" | "data", folder: string) =>
      choices.find((c) => c.key === folderKey(root, folder))?.count;

    expect(count("documents", "clients")).toBe(4);
    expect(count("documents", "clients/2025")).toBe(2);
    expect(count("data", "budget")).toBe(1);
    // A file at the top of its root is in no folder.
    expect(choices.some((c) => c.folder === "")).toBe(false);
  });

  it("flag a folder name that both roots hold, so the choice can say which root", () => {
    const named = choices.filter((c) => c.folder === "2025");

    expect(named).toHaveLength(2);
    expect(named.every((c) => c.ambiguous)).toBe(true);
    expect(choices.find((c) => c.folder === "budget")?.ambiguous).toBe(false);
  });

  it("cut at five and count the rest", () => {
    const many = mergeInventories(
      Array.from({ length: 9 }, (_, i) => record(`dir${i}/f.pdf`)),
      [],
    ).entries;
    const result = suggestFolders(folderChoices(many, none), "", new Set());

    expect(result.shown).toHaveLength(MAX_FOLDER_SUGGESTIONS);
    expect(result.more).toBe(4);
  });

  it("list the folders with the most files first", () => {
    const result = suggestFolders(choices, "", new Set());

    expect(result.shown[0]?.folder).toBe("clients");
    expect(result.shown.map((c) => c.count)).toStrictEqual(
      [...result.shown.map((c) => c.count)].sort((a, b) => b - a),
    );
  });

  it("match what was typed, ignoring case and accents, and leave out the ones already chosen", () => {
    const accented = folderChoices(
      mergeInventories([record("Contrôles/a.pdf"), record("Autre/b.pdf")], []).entries,
      none,
    );

    expect(suggestFolders(accented, "CONTROLES", new Set()).shown.map((c) => c.folder)).toStrictEqual([
      "Contrôles",
    ]);
    expect(
      suggestFolders(accented, "contr", new Set([folderKey("documents", "Contrôles")])).shown,
    ).toStrictEqual([]);
  });

  it("count the files the other filters leave, so a folder with none says 0", () => {
    const filter: ExplorerFilter = { states: new Set(["unreadable"]) };
    const counted = folderChoices(entries, filter);

    expect(counted.find((c) => c.key === folderKey("documents", "clients/2026"))?.count).toBe(1);
    expect(counted.find((c) => c.key === folderKey("documents", "clients/2025"))?.count).toBe(0);
  });
});

describe("the active pills", () => {
  const filter: ExplorerFilter = {
    formats: new Set(["pdf", "docx"]),
    states: new Set(["unreadable", "analysed"]),
    folders: new Set([folderKey("documents", "clients")]),
  };

  it("list the typed formats first, then the formats, the states in screen order, the folders", () => {
    expect(activePills(filter, ["xlsx"]).map((p) => `${p.kind}:${p.value}:${p.typed}`)).toStrictEqual([
      "format:xlsx:true",
      "format:pdf:false",
      "format:docx:false",
      "state:analysed:false",
      "state:unreadable:false",
      `folder:documents:clients:false`,
    ]);
  });

  it("show a format once when it is both typed and ticked, as the typed one", () => {
    const pills = activePills({ formats: new Set(["pdf"]) }, ["pdf"]);

    expect(pills).toStrictEqual([{ kind: "format", value: "pdf", typed: true }]);
  });

  it("count the choices in force for the badge", () => {
    expect(activeFilterCount(none, [])).toBe(0);
    expect(activeFilterCount(filter, ["xlsx"])).toBe(6);
  });

  it("are removed one at a time, a typed one leaving the sets alone", () => {
    const choices = {
      ...NO_CHOICES,
      formats: filter.formats ?? new Set<string>(),
      states: filter.states ?? new Set(),
      folders: filter.folders ?? new Set<string>(),
    };
    const pills = activePills(filter, ["xlsx"]);

    const withoutPdf = removePill(choices, pills.find((p) => p.value === "pdf")!);
    expect([...withoutPdf.formats]).toStrictEqual(["docx"]);
    const withoutFolder = removePill(choices, pills.find((p) => p.kind === "folder")!);
    expect(withoutFolder.folders.size).toBe(0);
    expect(removePill(choices, pills[0]!)).toBe(choices);
  });

  it("toggle a choice on and off", () => {
    const on = toggleChoice(NO_CHOICES, "state", "analysed");
    expect(on.states.has("analysed")).toBe(true);
    expect(toggleChoice(on, "state", "analysed").states.size).toBe(0);
    expect(toggleChoice(NO_CHOICES, "folder", "data:budget").folders.has("data:budget")).toBe(true);
  });

  it("read the folder out of its key", () => {
    expect(folderOfKey(folderKey("data", "a/b"))).toBe("a/b");
  });
});

describe("taking a typed extension back out of the text", () => {
  it("removes the word with or without the dot, whatever its case", () => {
    expect(withoutTypedFormat("rapport .docx", "docx")).toBe("rapport");
    expect(withoutTypedFormat("DOCX rapport", "docx")).toBe("rapport");
    expect(withoutTypedFormat(".xlsx", "xlsx")).toBe("");
  });

  it("leaves the other words, even one that only contains the extension", () => {
    expect(withoutTypedFormat("rapport docx2 pdf", "docx")).toBe("rapport docx2 pdf");
    expect(withoutTypedFormats("a .docx b xlsx", ["docx", "xlsx"])).toBe("a b");
  });
});

describe("a filter never changes the selection", () => {
  it("only narrows what 'tick the N' would add, and leaves both scopes as they were", () => {
    const scopes = { documents: noDocumentsScope(1), data: noDocumentsScope(1) };
    const filter: ExplorerFilter = {
      formats: new Set(["pdf"]),
      folders: new Set([folderKey("documents", "clients")]),
    };
    const shown = filterEntries(entries, filter);
    const toTick = entriesToTick(shown, entries, scopes);

    expect(toTick.map(keyOf)).toStrictEqual(["documents:clients/2025/a.pdf"]);
    expect(selectionOf(scopes.documents)).toBe("none");
    expect(selectionOf(scopes.data)).toBe("none");
    // Only the explicit act changes the scopes.
    const next = addEntriesToScopes(scopes, toTick, entries, 2);
    expect(selectionOf(next.documents)).not.toBe("none");
  });
});

describe("the reasons of a hit", () => {
  it("map to short labels, each once, where a person looks first", () => {
    expect(
      reasonKeys([
        { kind: "word", detail: "passage" },
        { kind: "word", detail: "name" },
        { kind: "word", detail: "name" },
        { kind: "word", detail: "folder" },
        { kind: "word", detail: "cells" },
      ]),
    ).toStrictEqual([
      "explorer.reason.name",
      "explorer.reason.folder",
      "explorer.reason.passage",
      "explorer.reason.cells",
    ]);
  });

  it("cover the other kinds, and say nothing for a detail it does not know", () => {
    expect(
      reasonKeys([
        { kind: "identifier", detail: null },
        { kind: "date", detail: "name" },
        { kind: "date", detail: "profile" },
        { kind: "entity", detail: "mention" },
        { kind: "entity", detail: "patient" },
        { kind: "word", detail: "something-new" },
        { kind: "word", detail: null },
      ]),
    ).toStrictEqual([
      "explorer.reason.identifier",
      "explorer.reason.mention",
      "explorer.reason.entity",
      "explorer.reason.dateName",
      "explorer.reason.date",
    ]);
  });
});
