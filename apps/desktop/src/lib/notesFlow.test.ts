import { describe, expect, it, vi } from "vitest";
import type { NoteDraft, NoteFile } from "../speech/ipc";
import type { FileRecord } from "./ipc";
import { saveAndAnalyse, type SaveDeps } from "./notesFlow";

function record(relativePath: string, processingStatus: FileRecord["processingStatus"]): FileRecord {
  return {
    id: relativePath,
    relativePath,
    name: relativePath,
    stem: relativePath,
    extension: "md",
    kind: "document_text",
    mimeType: "text/markdown",
    sizeBytes: 1,
    modifiedAt: null,
    sha256: null,
    readability: "not_assessed",
    processingStatus,
    extractionMethod: "none",
    indexed: processingStatus === "indexed",
    indexMetadata: null,
  };
}

const NOTE: NoteFile = {
  relativePath: "Notes/2026-10-18_14-32-05.md",
  fileName: "2026-10-18_14-32-05.md",
  bytes: 5,
  chars: 5,
  lines: 1,
};
const DRAFTS: NoteDraft[] = [{ name: "", text: "Salut", format: "md" }];

function deps(overrides: Partial<SaveDeps> = {}) {
  const calls: string[] = [];
  const all: SaveDeps = {
    save: vi.fn(async () => {
      calls.push("save");
      return [NOTE];
    }),
    inventory: vi.fn(async () => {
      calls.push("inventory");
      return [record(NOTE.relativePath, "discovered")];
    }),
    written: vi.fn(() => {
      calls.push("written");
    }),
    confirmAnalysis: vi.fn(async () => {
      calls.push("confirm");
      return true;
    }),
    analyse: vi.fn(async () => {
      calls.push("analyse");
      return true;
    }),
    ...overrides,
  };
  return { all, calls };
}

describe("saving a note, then analysing it", () => {
  it("writes once, marks the notes clean, then runs the Analyse pass once, without asking", async () => {
    const { all, calls } = deps();

    const report = await saveAndAnalyse(all, DRAFTS);

    expect(report).toStrictEqual({ written: [NOTE], analysis: "done" });
    expect(calls).toStrictEqual(["save", "written", "inventory", "analyse"]);
    expect(all.save).toHaveBeenCalledTimes(1);
    expect(all.save).toHaveBeenCalledWith(DRAFTS);
    expect(all.analyse).toHaveBeenCalledTimes(1);
    expect(all.confirmAnalysis).not.toHaveBeenCalled();
  });

  it("asks first when other documents are also pending, and analyses on a yes", async () => {
    const { all, calls } = deps({
      inventory: vi.fn(async () => [
        record(NOTE.relativePath, "discovered"),
        record("a.md", "pending"),
      ]),
    });

    const report = await saveAndAnalyse(all, DRAFTS);

    expect(report.analysis).toBe("done");
    expect(calls).toStrictEqual(["save", "written", "confirm", "analyse"]);
  });

  it("keeps the save and runs no pass when the user declines", async () => {
    const { all } = deps({
      inventory: vi.fn(async () => [record("a.md", "discovered")]),
      confirmAnalysis: vi.fn(async () => false),
    });

    const report = await saveAndAnalyse(all, DRAFTS);

    expect(report).toStrictEqual({ written: [NOTE], analysis: "declined" });
    expect(all.analyse).not.toHaveBeenCalled();
    expect(all.written).toHaveBeenCalledTimes(1);
  });

  it("asks when the folder cannot be read, because the count is unknown", async () => {
    const { all } = deps({
      inventory: vi.fn(async () => {
        throw new Error("unreadable");
      }),
    });

    await saveAndAnalyse(all, DRAFTS);

    expect(all.confirmAnalysis).toHaveBeenCalledTimes(1);
  });

  it("reports a failed pass without undoing the save", async () => {
    const { all } = deps({ analyse: vi.fn(async () => false) });

    const report = await saveAndAnalyse(all, DRAFTS);

    expect(report).toStrictEqual({ written: [NOTE], analysis: "failed" });
    expect(all.written).toHaveBeenCalledTimes(1);
  });

  it("treats a pass that throws as a failed pass", async () => {
    const { all } = deps({
      analyse: vi.fn(async () => {
        throw new Error("boom");
      }),
    });

    expect((await saveAndAnalyse(all, DRAFTS)).analysis).toBe("failed");
  });

  it("runs nothing after a save that failed: no clean note, no pass", async () => {
    const { all } = deps({
      save: vi.fn(async () => {
        throw { code: "notes_write_failed", data: {} };
      }),
    });

    await expect(saveAndAnalyse(all, DRAFTS)).rejects.toMatchObject({ code: "notes_write_failed" });
    expect(all.written).not.toHaveBeenCalled();
    expect(all.analyse).not.toHaveBeenCalled();
    expect(all.inventory).not.toHaveBeenCalled();
  });
});
