import { describe, expect, it } from "vitest";
import {
  AUTO_OFF,
  autoStep,
  modeOf,
  restoreCount,
  showsTickLink,
  targetOf,
  unreachableCount,
  type AutoEvent,
  type AutoState,
  type AutoTarget,
} from "./autoSelect";
import { selectionOf, tickedPaths } from "./analysisScope";
import { mergeInventories, type ExplorerEntry } from "./explorerModel";
import { scopedPaths } from "./analysisScope";
import { clearedScopes, tickableRecords, type ScopesByFamily } from "./explorerSelection";
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

/* Documents folder: four analysed, one not analysed, one unreadable. Data folder: two workbooks. */
const entries = mergeInventories(
  [
    record("a.pdf"),
    record("b.pdf"),
    record("c.pdf"),
    record("d.pdf"),
    record("e.pdf", "discovered"),
    record("f.pdf", "failed"),
  ],
  [record("t1.csv"), record("t2.csv")],
).entries;
const named = (...names: string[]): ExplorerEntry[] =>
  entries.filter((entry) => names.includes(entry.name));

const NOW = 1000;
const target = (...names: string[]): AutoTarget => targetOf(named(...names), entries, NOW);
const documentsOf = (scopes: ScopesByFamily) =>
  tickedPaths(scopes.documents, tickableRecords(entries, "documents")).sort();

function run(events: AutoEvent[], from: AutoState = AUTO_OFF) {
  let state = from;
  let applied: ScopesByFamily | null = null;
  const applications: (ScopesByFamily | null)[] = [];
  for (const event of events) {
    const step = autoStep(state, event);
    state = step.state;
    applications.push(step.apply);
    applied = step.apply ?? applied;
  }
  return { state, applied, applications };
}

/* The selection before the switch is turned on: only c.pdf ticked. */
const before: ScopesByFamily = targetOf(named("c.pdf"), entries, NOW).scopes;
const switchOn = (...names: string[]): AutoEvent => ({
  type: "switchOn",
  scopes: before,
  count: 1,
  target: target(...names),
});

describe("the automatic selection", () => {
  it("is off at the start and says the mode is manual", () => {
    expect(AUTO_OFF.on).toBe(false);
    expect(modeOf(AUTO_OFF)).toBe("manual");
    expect(restoreCount(AUTO_OFF)).toBeNull();
  });

  it("turns on with one snapshot of the selection as it was, and follows the result at once", () => {
    const { state, applied } = run([switchOn("a.pdf", "b.pdf")]);

    expect(state.on).toBe(true);
    expect(modeOf(state)).toBe("automatic");
    expect(state.snapshot?.scopes).toBe(before);
    expect(restoreCount(state)).toBe(1);
    expect(applied && documentsOf(applied)).toStrictEqual(["a.pdf", "b.pdf"]);
  });

  it("follows each new result once it arrives and not the same result twice", () => {
    const first = run([switchOn("a.pdf")]);
    const next = run([{ type: "resultArrived", target: target("a.pdf", "d.pdf") }], first.state);
    const again = run([{ type: "resultArrived", target: target("d.pdf", "a.pdf") }], next.state);

    expect(next.applied && documentsOf(next.applied)).toStrictEqual(["a.pdf", "d.pdf"]);
    expect(again.applied).toBeNull();
  });

  it("selects only the analysed files of a result and counts the others as not tickable", () => {
    const shown = named("a.pdf", "e.pdf", "f.pdf");
    const { scopes } = targetOf(shown, entries, NOW);

    expect(documentsOf(scopes)).toStrictEqual(["a.pdf"]);
    expect(unreachableCount(shown)).toBe(2);
    expect(selectionOf(scopes.data)).toBe("none");
  });

  it("gives an empty selection for an empty result", () => {
    const { state, applied } = run([
      switchOn("a.pdf"),
      { type: "resultArrived", target: target() },
    ]);

    expect(state.on).toBe(true);
    expect(applied && selectionOf(applied.documents)).toBe("none");
    expect(applied && selectionOf(applied.data)).toBe("none");
  });

  it("ticks a file of the other folder once it is analysed and splits the selection by family", () => {
    const all = mergeInventories(
      [record("a.pdf"), record("grid.xlsx"), record("plan.xlsx", "discovered")],
      [record("t1.csv"), record("note.docx"), record("brouillon.docx", "discovered")],
    ).entries;
    const shown = all.filter((entry) => ["grid.xlsx", "note.docx", "plan.xlsx"].includes(entry.name));
    const { scopes } = targetOf(shown, all, NOW);

    // The workbook of the Documents folder goes to the tables, the document of the Data folder to the
    // documents; each under the key the index knows it by.
    expect(scopedPaths(scopes.data)).toStrictEqual(["/documents/grid.xlsx"]);
    expect(scopedPaths(scopes.documents)).toStrictEqual(["/data/note.docx"]);
    // The one not analysed yet is not ticked, and the bar counts it.
    expect(unreachableCount(shown)).toBe(1);
  });

  it("gives a result with no filter at all as every analysed file, as any ticking does", () => {
    const { scopes } = targetOf(entries, entries, NOW);

    expect(selectionOf(scopes.documents)).toBe("all");
    expect(selectionOf(scopes.data)).toBe("all");
  });

  it("does nothing on a result while it is off", () => {
    const { state, applied } = run([{ type: "resultArrived", target: target("a.pdf") }]);

    expect(state).toStrictEqual(AUTO_OFF);
    expect(applied).toBeNull();
  });

  it("turns off on a manual change and keeps the snapshot", () => {
    const { state, applied } = run([switchOn("a.pdf"), { type: "manualChange" }]);

    expect(state.on).toBe(false);
    expect(state.snapshot?.scopes).toBe(before);
    expect(restoreCount(state)).toBe(1);
    // Nothing is applied by the manual change itself: the caller made the change.
    expect(applied && documentsOf(applied)).toStrictEqual(["a.pdf"]);
  });

  it("no longer follows a result once a manual change turned it off", () => {
    const { applications } = run([
      switchOn("a.pdf"),
      { type: "manualChange" },
      { type: "resultArrived", target: target("b.pdf") },
    ]);

    expect(applications[2]).toBeNull();
  });

  it("restores the snapshot, turns off and forgets the snapshot", () => {
    const { state, applied } = run([switchOn("a.pdf", "b.pdf"), { type: "restore" }]);

    expect(state.on).toBe(false);
    expect(state.snapshot).toBeNull();
    expect(applied).toBe(before);
    expect(applied && documentsOf(applied)).toStrictEqual(["c.pdf"]);
  });

  it("restores after a manual change too, because the snapshot stays", () => {
    const { applied } = run([switchOn("a.pdf"), { type: "manualChange" }, { type: "restore" }]);

    expect(applied).toBe(before);
  });

  it("restores nothing when there is no snapshot", () => {
    expect(autoStep(AUTO_OFF, { type: "restore" })).toStrictEqual({ state: AUTO_OFF, apply: null });
  });

  it("turns off with the switch and leaves the selection as it is", () => {
    const { state, applications } = run([switchOn("a.pdf"), { type: "switchOff" }]);

    expect(state.on).toBe(false);
    expect(applications[1]).toBeNull();
    expect(restoreCount(state)).toBe(1);
  });

  it("takes a new snapshot each time it is turned on", () => {
    const second: ScopesByFamily = clearedScopes(NOW);
    const { state } = run([
      switchOn("a.pdf"),
      { type: "switchOff" },
      { type: "switchOn", scopes: second, count: 0, target: target("b.pdf") },
    ]);

    expect(state.snapshot?.scopes).toBe(second);
    expect(restoreCount(state)).toBe(0);
  });

  it("is locked while a question is being answered: nothing changes under the answer", () => {
    const on = run([switchOn("a.pdf"), { type: "questionStarted" }]).state;

    for (const event of [
      { type: "switchOff" },
      { type: "manualChange" },
      { type: "restore" },
    ] as AutoEvent[]) {
      const step = autoStep(on, event);
      expect(step.state).toBe(on);
      expect(step.apply).toBeNull();
    }
    const asked = autoStep(on, { type: "resultArrived", target: target("b.pdf") });
    expect(asked.apply).toBeNull();
    const off = autoStep(AUTO_OFF, { type: "questionStarted" }).state;
    expect(autoStep(off, switchOn("a.pdf")).state.on).toBe(false);
  });

  it("follows the result that arrived during a question when the question ends", () => {
    const { state, applied } = run([
      switchOn("a.pdf"),
      { type: "questionStarted" },
      { type: "resultArrived", target: target("b.pdf") },
      { type: "resultArrived", target: target("b.pdf", "d.pdf") },
      { type: "questionEnded" },
    ]);

    expect(state.locked).toBe(false);
    expect(state.deferred).toBeNull();
    expect(applied && documentsOf(applied)).toStrictEqual(["b.pdf", "d.pdf"]);
  });

  it("applies nothing when the question ends and the switch was off or the result unchanged", () => {
    const unchangedResult = run([
      switchOn("a.pdf"),
      { type: "questionStarted" },
      { type: "resultArrived", target: target("a.pdf") },
      { type: "questionEnded" },
    ]);
    expect(unchangedResult.applications[3]).toBeNull();

    const off = run([{ type: "questionStarted" }, { type: "questionEnded" }]);
    expect(off.applications[1]).toBeNull();
  });

  it("hides the tick link while on and shows it only when something can be added", () => {
    const on = run([switchOn("a.pdf")]).state;

    expect(showsTickLink(on, 3)).toBe(false);
    expect(showsTickLink(AUTO_OFF, 3)).toBe(true);
    expect(showsTickLink(AUTO_OFF, 0)).toBe(false);
  });

  it("keeps a result the same whatever the order of the files", () => {
    const one = targetOf(named("a.pdf", "b.pdf"), entries, NOW);
    const other = targetOf(named("b.pdf", "a.pdf"), entries, NOW);

    expect(one.key).toBe(other.key);
  });
});
