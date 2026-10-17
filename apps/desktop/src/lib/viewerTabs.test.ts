import { describe, expect, it } from "vitest";
import type { FileRecord } from "./ipc";
import {
  NO_TABS,
  activateTab,
  closeTab,
  closeTabs,
  idsOfFolder,
  openTab,
  tabId,
  tabLabel,
  tabsOf,
  viewOf,
  type TabsState,
} from "./viewerTabs";

function record(relativePath: string, sha256 = "hash-1"): FileRecord {
  return {
    id: relativePath,
    relativePath,
    name: relativePath,
    extension: "pdf",
    sha256,
    processingStatus: "indexed",
  } as FileRecord;
}

function opened(...paths: string[]): TabsState {
  return paths.reduce((state, path) => openTab(state, "documents", record(path)), NO_TABS);
}

const ids = (state: TabsState) => state.tabs.map((tab) => tab.id);

describe("tabId", () => {
  it("gives two ids to the same path in two folders", () => {
    expect(tabId("documents", "a.pdf")).not.toBe(tabId("data", "a.pdf"));
  });

  it("cannot collide across the folder and the path", () => {
    expect(tabId("documents", "x\\y")).not.toBe(tabId("documents", "x/y"));
    expect(tabId("documents", "a")).not.toBe(tabId("documents", "a "));
  });
});

describe("tabLabel", () => {
  it("is the last segment of the path, whatever the separator", () => {
    expect(tabLabel("a/b/c.pdf")).toBe("c.pdf");
    expect(tabLabel("a\\b\\c.pdf")).toBe("c.pdf");
    expect(tabLabel("c.pdf")).toBe("c.pdf");
  });
});

describe("openTab", () => {
  it("adds a tab at the end and activates it", () => {
    const state = opened("a.pdf", "b.pdf");
    expect(state.tabs.map((tab) => tab.record.relativePath)).toEqual(["a.pdf", "b.pdf"]);
    expect(state.activeId).toBe(tabId("documents", "b.pdf"));
  });

  it("activates an open file instead of duplicating it", () => {
    const state = openTab(opened("a.pdf", "b.pdf"), "documents", record("a.pdf"));
    expect(state.tabs).toHaveLength(2);
    expect(state.activeId).toBe(tabId("documents", "a.pdf"));
  });

  it("keeps the record when the content is the same", () => {
    const first = opened("a.pdf", "b.pdf");
    const again = openTab(first, "documents", record("a.pdf"));
    expect(again.tabs[0]).toBe(first.tabs[0]);
  });

  it("replaces the record when the content is another", () => {
    const state = openTab(opened("a.pdf", "b.pdf"), "documents", record("a.pdf", "hash-2"));
    expect(state.tabs).toHaveLength(2);
    expect(state.tabs[0]?.record.sha256).toBe("hash-2");
    expect(state.activeId).toBe(tabId("documents", "a.pdf"));
  });

  it("gives two tabs to the same name in the two folders", () => {
    const state = openTab(opened("a.pdf"), "data", record("a.pdf"));
    expect(state.tabs.map((tab) => tab.folder)).toEqual(["documents", "data"]);
  });
});

describe("activateTab", () => {
  it("shows another open tab and ignores an id that is not open", () => {
    const state = opened("a.pdf", "b.pdf");
    expect(activateTab(state, tabId("documents", "a.pdf")).activeId).toBe(
      tabId("documents", "a.pdf"),
    );
    expect(activateTab(state, "nothing")).toBe(state);
  });
});

describe("closeTab", () => {
  const a = tabId("documents", "a.pdf");
  const b = tabId("documents", "b.pdf");
  const c = tabId("documents", "c.pdf");

  it("hands the active tab to its right neighbour", () => {
    const state = activateTab(opened("a.pdf", "b.pdf", "c.pdf"), b);
    const after = closeTab(state, b);
    expect(ids(after)).toEqual([a, c]);
    expect(after.activeId).toBe(c);
  });

  it("falls back to the left neighbour when the last tab is closed", () => {
    const after = closeTab(opened("a.pdf", "b.pdf", "c.pdf"), c);
    expect(after.activeId).toBe(b);
  });

  it("leaves the active tab alone when another one is closed", () => {
    const after = closeTab(opened("a.pdf", "b.pdf", "c.pdf"), a);
    expect(ids(after)).toEqual([b, c]);
    expect(after.activeId).toBe(c);
  });

  it("empties the list when the last tab is closed", () => {
    expect(closeTab(opened("a.pdf"), a)).toBe(NO_TABS);
  });

  it("changes nothing for an id that is not open", () => {
    const state = opened("a.pdf");
    expect(closeTab(state, "nothing")).toBe(state);
  });
});

describe("closeTabs", () => {
  const a = tabId("documents", "a.pdf");
  const b = tabId("documents", "b.pdf");
  const c = tabId("documents", "c.pdf");
  const d = tabId("documents", "d.pdf");

  it("closes several at once and moves the active tab to a neighbour that stays", () => {
    const state = activateTab(opened("a.pdf", "b.pdf", "c.pdf", "d.pdf"), b);
    const after = closeTabs(state, [b, c]);
    expect(ids(after)).toEqual([a, d]);
    expect(after.activeId).toBe(d);
  });

  it("falls back to the left when nothing stays on the right", () => {
    const after = closeTabs(opened("a.pdf", "b.pdf", "c.pdf", "d.pdf"), [c, d]);
    expect(after.activeId).toBe(b);
  });

  it("keeps the active tab when it is not among those closed", () => {
    const after = closeTabs(opened("a.pdf", "b.pdf", "c.pdf", "d.pdf"), [a, b]);
    expect(after.activeId).toBe(d);
  });

  it("empties the list when every tab goes", () => {
    expect(closeTabs(opened("a.pdf", "b.pdf"), [a, b])).toBe(NO_TABS);
  });

  it("changes nothing for an empty list of ids", () => {
    const state = opened("a.pdf");
    expect(closeTabs(state, [])).toBe(state);
  });
});

describe("idsOfFolder", () => {
  it("lists the tabs of one folder only", () => {
    const state = openTab(opened("a.pdf"), "data", record("a.pdf"));
    expect(idsOfFolder(state, "data")).toEqual([tabId("data", "a.pdf")]);
  });
});

describe("the view of a tab list", () => {
  it("is closed (null) when no tab is left", () => {
    expect(viewOf(NO_TABS)).toBeNull();
  });

  it("round-trips through tabsOf", () => {
    const state = opened("a.pdf", "b.pdf");
    expect(tabsOf(viewOf(state))).toEqual(state);
  });

  it("holds no tab for the welcome view or a closed centre", () => {
    expect(tabsOf({ kind: "welcome" })).toBe(NO_TABS);
    expect(tabsOf(null)).toBe(NO_TABS);
  });
});
