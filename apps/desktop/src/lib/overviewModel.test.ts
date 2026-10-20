import { describe, expect, it } from "vitest";
import { mergeInventories, type ExplorerEntry } from "./explorerModel";
import type { FileRecord, IndexSummary, ProcessingStatus } from "./ipc";
import { NO_VIEW } from "./explorerView";
import {
  checkActions,
  checkView,
  MAX_CHECK_LISTED,
  overviewInput,
  overviewModel,
  type FamilyReportInput,
  type OverviewBlock,
  type OverviewInput,
} from "./overviewModel";

function record(
  relativePath: string,
  status: ProcessingStatus = "indexed",
  ocrEngine: string | null = null,
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
    modifiedAt: 1000,
    processingStatus: status,
    indexMetadata:
      status === "failed"
        ? { indexedSha256: "x", chunkCount: 0, ocrEngine, ocrEngineVersion: null }
        : null,
  } as FileRecord;
}

function summary(overrides: Partial<IndexSummary> = {}): IndexSummary {
  return {
    scannedFiles: 0,
    indexedFiles: 0,
    unchangedFiles: 0,
    emptyFiles: [],
    ocrFiles: [],
    lowConfidenceFiles: [],
    removedFiles: [],
    renamedFiles: [],
    renameFailedFiles: [],
    failedFiles: [],
    unavailableCapabilities: [],
    unreadFormats: [],
    chunkCount: 0,
    timings: null,
    ...overrides,
  };
}

const NONE: FamilyReportInput = {
  folder: null,
  folderState: "none",
  summary: null,
  durationMs: null,
  finishedAt: null,
  running: false,
  error: null,
  meaningPercent: null,
  needsAnalyse: false,
};

const FILLED: FamilyReportInput = { ...NONE, folder: "/folder", folderState: "filled" };

function input(
  documents: FileRecord[],
  data: FileRecord[],
  reports: { documents?: Partial<FamilyReportInput>; tables?: Partial<FamilyReportInput> } = {},
): OverviewInput {
  return {
    entries: mergeInventories(documents, data).entries,
    documents: { ...FILLED, ...reports.documents },
    tables: { ...FILLED, ...reports.tables },
  };
}

function blocksOf<K extends OverviewBlock["kind"]>(
  page: { blocks: OverviewBlock[] },
  kind: K,
): Extract<OverviewBlock, { kind: K }>[] {
  return page.blocks.filter((block): block is Extract<OverviewBlock, { kind: K }> => block.kind === kind);
}

describe("the page with no folder", () => {
  it("is the first-start guidance and nothing else", () => {
    const page = overviewModel({ entries: [], documents: NONE, tables: NONE });

    expect(page.phase).toBe("firstStart");
    expect(page.blocks).toStrictEqual([{ kind: "firstStart" }]);
  });
});

describe("the page before the first analysis", () => {
  it("gives the steps and one card per family with the files that wait", () => {
    const page = overviewModel(
      input(
        [record("a.pdf", "discovered"), record("b.docx", "discovered")],
        [record("t.csv", "discovered")],
      ),
    );

    expect(page.phase).toBe("beforeAnalysis");
    expect(blocksOf(page, "steps")).toStrictEqual([
      { kind: "steps", chosen: 2, filesFound: 3, running: false },
    ]);
    const cards = blocksOf(page, "familyCard");
    expect(cards.map((card) => [card.family, card.total, card.waiting])).toStrictEqual([
      ["documents", 2, 2],
      ["tables", 1, 1],
    ]);
    expect(blocksOf(page, "figures")).toHaveLength(0);
  });

  it("counts one folder chosen as one of two steps, and the empty one as no file", () => {
    const page = overviewModel({
      entries: [],
      documents: { ...FILLED, folderState: "empty" },
      tables: NONE,
    });

    expect(page.phase).toBe("beforeAnalysis");
    expect(blocksOf(page, "steps")[0]).toMatchObject({ chosen: 1, filesFound: 0 });
    const cards = blocksOf(page, "familyCard");
    expect(cards[0]).toMatchObject({ folderChosen: true, folderState: "empty", total: 0 });
    expect(cards[1]).toMatchObject({ folderChosen: false, folderState: "none" });
  });

  it("does not count a file of the other folder as waiting for its family", () => {
    const page = overviewModel(input([record("sheet.xlsx", "discovered")], []));

    expect(blocksOf(page, "familyCard").map((card) => [card.family, card.total, card.waiting])).toStrictEqual([
      ["documents", 0, 0],
      ["tables", 1, 0],
    ]);
  });

  it("says the first pass is running", () => {
    const page = overviewModel(
      input([record("a.pdf", "discovered")], [], { documents: { running: true } }),
    );

    expect(blocksOf(page, "steps")[0]?.running).toBe(true);
  });

  it("names a first pass that failed before it reported", () => {
    const error = { code: "gateway_unreachable", data: {} };
    const page = overviewModel(input([record("a.pdf", "discovered")], [], { documents: { error } }));

    expect(page.phase).toBe("beforeAnalysis");
    expect(blocksOf(page, "passFailed")).toStrictEqual([
      { kind: "passFailed", family: "documents", error },
    ]);
  });
});

describe("the page after an analysis", () => {
  it("gives the figures of each family from the files, then the last pass", () => {
    const page = overviewModel(
      input(
        [record("a.pdf"), record("b.pdf"), record("c.pdf", "failed"), record("d.pdf", "discovered")],
        [record("t.csv")],
        {
          documents: {
            summary: summary({ indexedFiles: 2 }),
            durationMs: 409_000,
            finishedAt: 5000,
            meaningPercent: 63,
          },
        },
      ),
    );

    expect(page.phase).toBe("report");
    const [figures] = blocksOf(page, "figures");
    expect(figures?.families).toStrictEqual([
      {
        family: "documents",
        folder: "/folder",
        folderState: "filled",
        running: false,
        analysed: 2,
        unreadable: 1,
        toAnalyse: 1,
        meaningPercent: 63,
        needsAnalyse: false,
        lastPass: { at: 5000, durationMs: 409_000, outcome: "done" },
      },
      {
        family: "tables",
        folder: "/folder",
        folderState: "filled",
        running: false,
        analysed: 1,
        unreadable: 0,
        toAnalyse: 0,
        meaningPercent: null,
        needsAnalyse: false,
        lastPass: null,
      },
    ]);
  });

  it("reads an index analysed before this session without a last pass", () => {
    const page = overviewModel(input([record("a.pdf")], []));

    expect(page.phase).toBe("report");
    expect(blocksOf(page, "figures")[0]?.families[0]?.lastPass).toBeNull();
  });

  it("lists only the families that have a folder", () => {
    const page = overviewModel({
      entries: mergeInventories([record("a.pdf")], []).entries,
      documents: FILLED,
      tables: NONE,
    });

    expect(blocksOf(page, "figures")[0]?.families.map((family) => family.family)).toStrictEqual([
      "documents",
    ]);
  });

  it("is clean when nothing is to check, to rename or to say", () => {
    const page = overviewModel(
      input([record("a.pdf")], [record("t.csv")], {
        documents: { summary: summary({ indexedFiles: 1 }) },
        tables: { summary: summary({ indexedFiles: 1 }) },
      }),
    );

    expect(page.blocks.map((block) => block.kind)).toStrictEqual(["figures"]);
  });

  it("says a stopped pass is stopped, and keeps the rest of what it reported", () => {
    const page = overviewModel(
      input([record("a.pdf")], [], {
        documents: {
          summary: summary({
            cancelled: true,
            indexedFiles: 40,
            renamedFiles: [{ from: "Mon fichier.pdf", to: "Mon-fichier.pdf" }],
          }),
          durationMs: 228_000,
          finishedAt: 7,
        },
      }),
    );

    expect(blocksOf(page, "figures")[0]?.families[0]?.lastPass?.outcome).toBe("stopped");
    expect(blocksOf(page, "renames")).toHaveLength(1);
    expect(blocksOf(page, "knowledge")).toHaveLength(0);
  });

  it("says a pass with nothing new was a pass with nothing new", () => {
    const page = overviewModel(
      input([record("a.pdf")], [], { documents: { summary: summary({ unchangedFiles: 1 }) } }),
    );

    expect(blocksOf(page, "figures")[0]?.families[0]?.lastPass?.outcome).toBe("nothing");
  });

  it("keeps the figures and names the failure of a pass that produced no summary", () => {
    const error = { code: "gateway_unreachable", data: {} };
    const page = overviewModel(input([record("a.pdf")], [], { documents: { error } }));

    expect(page.phase).toBe("report");
    expect(blocksOf(page, "passFailed")).toHaveLength(1);
    expect(blocksOf(page, "figures")).toHaveLength(1);
  });

  it("does not repeat the failure once a pass has reported", () => {
    const error = { code: "gateway_unreachable", data: {} };
    const page = overviewModel(
      input([record("a.pdf")], [], { documents: { error, summary: summary({ indexedFiles: 1 }) } }),
    );

    expect(blocksOf(page, "passFailed")).toHaveLength(0);
  });
});

describe("the files to check", () => {
  const failing = summary({
    failedFiles: [{ path: "scan.pdf", code: "gateway_unreachable", data: { reason: "x" } }],
    emptyFiles: ["blank.pdf", "faint.pdf"],
    lowConfidenceFiles: ["faint.pdf"],
  });

  it("joins the unreadable files and the ones the pass failed to read, with the reason of each", () => {
    const page = overviewModel(
      input(
        [
          record("blank.pdf", "failed"),
          record("faint.pdf", "failed"),
          record("scan.pdf", "discovered"),
          record("ok.pdf"),
        ],
        [record("broken.csv", "failed")],
        { documents: { summary: failing } },
      ),
    );

    const [check] = blocksOf(page, "check");
    expect(check?.total).toBe(4);
    expect(check?.items.map((item) => [item.entry.name, item.reason.kind])).toStrictEqual([
      ["blank.pdf", "empty"],
      ["broken.csv", "noTable"],
      ["faint.pdf", "lowConfidence"],
      ["scan.pdf", "error"],
    ]);
    expect(check?.items.find((item) => item.entry.name === "scan.pdf")?.reason).toStrictEqual({
      kind: "error",
      code: "gateway_unreachable",
      data: { reason: "x" },
    });
  });

  it("counts for the link only what the state filter of the left list shows", () => {
    const page = overviewModel(
      input([record("blank.pdf", "failed"), record("scan.pdf", "discovered")], [], {
        documents: { summary: failing },
      }),
    );

    const [check] = blocksOf(page, "check");
    expect(check?.total).toBe(2);
    expect(check?.inList).toBe(1);
  });

  it("says why an unreadable file is unreadable from the index when no pass of this session did", () => {
    const page = overviewModel(
      input(
        [
          record("scan.pdf", "failed", "tesseract"),
          record("blank.txt", "failed"),
          record("ok.pdf"),
        ],
        [record("broken.csv", "failed")],
      ),
    );

    const reasons = Object.fromEntries(
      (blocksOf(page, "check")[0]?.items ?? []).map((item) => [item.entry.name, item.reason]),
    );
    // Text recognition ran on the scan and found nothing; nothing was recognised in the empty text file.
    expect(reasons["scan.pdf"]).toStrictEqual({ kind: "recognised" });
    expect(reasons["blank.txt"]).toStrictEqual({ kind: "empty" });
    expect(reasons["broken.csv"]).toStrictEqual({ kind: "noTable" });
  });

  it("never leaves an unreadable file without a reason", () => {
    const page = overviewModel(
      input([record("a.pdf", "failed"), record("b.docx", "failed", "tesseract")], [record("t.csv", "failed")]),
    );

    for (const item of blocksOf(page, "check")[0]?.items ?? []) {
      expect(["error", "empty", "recognised", "lowConfidence", "noTable"]).toContain(item.reason.kind);
    }
  });

  it("gives the whole path of each family's folder in the cards and the figures", () => {
    const before = overviewModel(input([record("a.pdf", "discovered")], [record("t.csv", "discovered")], {
      documents: { folder: "C:\\Users\\x\\Docs" },
      tables: { folder: "C:\\Users\\x\\Data" },
    }));
    expect(blocksOf(before, "familyCard").map((card) => card.folder)).toStrictEqual([
      "C:\\Users\\x\\Docs",
      "C:\\Users\\x\\Data",
    ]);
    const after = overviewModel(input([record("a.pdf")], [], { documents: { folder: "/home/x/Docs" } }));
    expect(blocksOf(after, "figures")[0]?.families.map((family) => family.folder)).toStrictEqual([
      "/home/x/Docs",
      "/folder",
    ]);
  });

  it("matches the paths of a pass whatever the separator", () => {
    const page = overviewModel(
      input([record("sub/blank.pdf", "failed")], [], {
        documents: { summary: summary({ emptyFiles: ["sub\\blank.pdf"] }) },
      }),
    );

    expect(blocksOf(page, "check")[0]?.items[0]?.reason.kind).toBe("empty");
  });

  it("lists a limited number and counts the rest", () => {
    const many = Array.from({ length: MAX_CHECK_LISTED + 5 }, (_, index) =>
      record(`f${String(index).padStart(3, "0")}.pdf`, "failed"),
    );
    const page = overviewModel(input(many, []));

    const [check] = blocksOf(page, "check");
    expect(check?.items).toHaveLength(MAX_CHECK_LISTED);
    expect(check?.more).toBe(5);
    expect(check?.total).toBe(MAX_CHECK_LISTED + 5);
  });

  it("has no block when nothing is to check", () => {
    expect(blocksOf(overviewModel(input([record("a.pdf")], [])), "check")).toHaveLength(0);
  });

  it("leaves out a file of the other folder: its own pipeline did not run on it", () => {
    const page = overviewModel(input([record("sheet.xlsx", "failed"), record("a.pdf")], []));

    expect(blocksOf(page, "check")).toHaveLength(0);
  });
});

describe("what the link and the buttons do", () => {
  function entryOf(path: string, status: ProcessingStatus, root: "documents" | "data" = "documents") {
    const merged: ExplorerEntry[] =
      root === "documents"
        ? mergeInventories([record(path, status)], []).entries
        : mergeInventories([], [record(path, status)]).entries;
    return merged[0] as ExplorerEntry;
  }

  it("filters the left list to the unreadable files of both families, whatever was set before", () => {
    const view = checkView();

    expect(view.family).toBe("all");
    expect([...view.choices.states]).toStrictEqual(["unreadable"]);
    expect(view.choices.formats.size).toBe(0);
    expect(view.choices.folders.size).toBe(0);
    expect(view).not.toBe(NO_VIEW);
  });

  it("opens only an analysed file of its own folder, and reveals any file of its own folder", () => {
    expect(checkActions(entryOf("a.pdf", "failed"))).toStrictEqual({ open: false, reveal: true });
    expect(checkActions(entryOf("a.pdf", "indexed"))).toStrictEqual({ open: true, reveal: true });
    expect(checkActions(entryOf("sheet.xlsx", "failed"))).toStrictEqual({ open: false, reveal: false });
    expect(checkActions(entryOf("t.csv", "failed", "data"))).toStrictEqual({ open: false, reveal: true });
  });
});

describe("what the passes added", () => {
  it("lists the cleaned names in full, never five", () => {
    const renamedFiles = Array.from({ length: 12 }, (_, index) => ({
      from: `Mon fichier ${index}.pdf`,
      to: `Mon-fichier-${index}.pdf`,
    }));
    const page = overviewModel(
      input([record("a.pdf")], [], {
        documents: { summary: summary({ renamedFiles, renameFailedFiles: ["x y.pdf"] }) },
      }),
    );

    const [renames] = blocksOf(page, "renames");
    expect(renames?.items).toHaveLength(12);
    expect(renames?.failed).toBe(1);
  });

  it("counts the files a pass dropped because they left the folder, per family", () => {
    const page = overviewModel(
      input([record("a.pdf")], [record("t.csv")], {
        documents: { summary: summary({ removedFiles: ["x.pdf", "y.pdf"] }) },
        tables: { summary: summary({ removedFiles: Array.from({ length: 510 }, (_, i) => `w${i}.csv`) }) },
      }),
    );

    expect(blocksOf(page, "removed")).toStrictEqual([
      { kind: "removed", family: "documents", count: 2 },
      { kind: "removed", family: "tables", count: 510 },
    ]);
  });

  it("lists the files read by text recognition and the unread formats per family", () => {
    const page = overviewModel(
      input([record("a.pdf")], [record("t.csv")], {
        documents: {
          summary: summary({
            ocrFiles: ["scan.pdf", "photo.jpg"],
            unreadFormats: [
              { extension: "ppt", count: 1 },
              { extension: "key", count: 2 },
            ],
          }),
        },
        tables: { summary: summary({ unreadFormats: [{ extension: "numbers", count: 1 }] }) },
      }),
    );

    expect(blocksOf(page, "ocr")[0]?.files).toStrictEqual(["scan.pdf", "photo.jpg"]);
    expect(blocksOf(page, "unreadFormats").map((block) => [block.family, block.total])).toStrictEqual([
      ["documents", 3],
      ["tables", 1],
    ]);
  });

  it("carries the knowledge counts of the pass that took part and none from a pass that did not", () => {
    const knowledge = {
      entitiesDetected: 4,
      matchedExisting: 1,
      createdNew: 3,
      candidates: 0,
      identifiers: 2,
      relations: 0,
      semanticColumns: 0,
      workbooksAnalysed: 0,
      refreshedSources: 0,
      truncatedSources: 0,
      errors: 0,
      elapsedMs: 10,
    };
    const page = overviewModel(
      input([record("a.pdf")], [record("t.csv")], {
        documents: { summary: summary({ knowledge }) },
        tables: { summary: summary({ knowledge: null }) },
      }),
    );

    expect(blocksOf(page, "knowledge").map((block) => block.family)).toStrictEqual(["documents"]);
  });

  it("names a capability that did not start once, and drops a code it does not know", () => {
    const page = overviewModel(
      input([record("a.pdf")], [record("t.csv")], {
        documents: { summary: summary({ unavailableCapabilities: ["ocrEngine", "somethingNew"] }) },
        tables: { summary: summary({ unavailableCapabilities: ["ocrEngine"] }) },
      }),
    );

    expect(blocksOf(page, "capabilities")).toStrictEqual([
      { kind: "capabilities", codes: ["ocrEngine"] },
    ]);
  });

  it("tells where the time went, leaving out the stages under half a second", () => {
    const timings = {
      pass: "documents" as const,
      filesScanned: 3,
      filesProcessed: 3,
      hashMs: 200,
      extractMs: 5000,
      ocrMs: 120_000,
      chunkMs: 100,
      embedMs: 200_000,
      embedBatches: 4,
      embeddedChars: 10,
      writeMs: 400,
      buildInventoryMs: 0,
      chunksTotal: 3,
      chunksRepeatingEarlierText: 0,
      totalMs: 330_000,
      slowestFiles: [],
    };
    const page = overviewModel(
      input([record("a.pdf")], [], {
        documents: { summary: summary({ timings }), durationMs: 409_000 },
      }),
    );

    expect(blocksOf(page, "time")).toStrictEqual([
      {
        kind: "time",
        family: "documents",
        totalMs: 409_000,
        parts: [
          { id: "read", ms: 5200 },
          { id: "recognition", ms: 120_000 },
          { id: "meaning", ms: 200_000 },
        ],
      },
    ]);
  });

  it("uses the timings' total when the client did not measure the pass, and shows no time at zero", () => {
    const timings = {
      pass: "data" as const,
      filesScanned: 1,
      filesProcessed: 1,
      hashMs: 0,
      extractMs: 0,
      ocrMs: 0,
      chunkMs: 0,
      embedMs: 0,
      embedBatches: 0,
      embeddedChars: 0,
      writeMs: 30,
      buildInventoryMs: 900,
      chunksTotal: 0,
      chunksRepeatingEarlierText: 0,
      totalMs: 1000,
      slowestFiles: [],
    };
    const page = overviewModel(
      input([record("a.pdf")], [record("t.csv")], {
        documents: { summary: summary() },
        tables: { summary: summary({ timings }) },
      }),
    );

    expect(blocksOf(page, "time")).toStrictEqual([
      { kind: "time", family: "tables", totalMs: 1000, parts: [{ id: "inventory", ms: 900 }] },
    ]);
  });

  it("orders the blocks: figures, failures, files to check, then what the passes added", () => {
    const page = overviewModel(
      input([record("a.pdf"), record("b.pdf", "failed")], [], {
        documents: {
          summary: summary({
            removedFiles: ["gone.pdf"],
            renamedFiles: [{ from: "x y.pdf", to: "x-y.pdf" }],
            ocrFiles: ["a.pdf"],
            unreadFormats: [{ extension: "ppt", count: 1 }],
            unavailableCapabilities: ["ocrEngine"],
          }),
          durationMs: 1000,
        },
      }),
    );

    expect(page.blocks.map((block) => block.kind)).toStrictEqual([
      "figures",
      "check",
      "removed",
      "renames",
      "ocr",
      "unreadFormats",
      "capabilities",
      "time",
    ]);
  });
});

describe("the page's input from the explorer and the passes", () => {
  const pass = {
    summary: null,
    durationMs: null,
    finishedAt: null,
    running: false,
    error: null,
  };
  const folders = { documents: "/docs", tables: null };

  it("assumes a chosen folder holds files until the explorer has reported", () => {
    const built = overviewInput(null, folders, pass, pass);

    expect(built.documents).toMatchObject({ folder: "/docs", folderState: "filled" });
    expect(built.tables).toMatchObject({ folder: null, folderState: "none" });
    expect(built.entries).toStrictEqual([]);
  });

  it("takes the situations, the meaning share and the profile notes from the explorer", () => {
    const built = overviewInput(
      {
        entries: [],
        documentsState: "empty",
        tablesState: "filled",
        meaningPercent: 40,
        documentsNeedsAnalyse: true,
        tablesNeedsAnalyse: false,
      },
      { documents: "/docs", tables: "/data" },
      pass,
      pass,
    );

    expect(built.documents).toMatchObject({
      folderState: "empty",
      meaningPercent: 40,
      needsAnalyse: true,
    });
    expect(built.tables).toMatchObject({ folderState: "filled", meaningPercent: null });
  });
});
