import type { FileRecord } from "./ipc";
import type { OpenFile, ViewerFolder, WorkspaceView } from "./workspaceView";

/** The tabs of the centre and the one shown. `activeId` is `null` only for an empty list. */
export interface TabsState {
  tabs: readonly OpenFile[];
  activeId: string | null;
}

export const NO_TABS: TabsState = { tabs: [], activeId: null };

/**
 * The identity of a tab: the folder and the path relative to it. The same name can exist in both
 * folders and must give two tabs. A NUL character cannot occur in a folder name or in a path, so
 * two different pairs never give the same text.
 */
export function tabId(folder: ViewerFolder, relativePath: string): string {
  return `${folder}\u0000${relativePath}`;
}

/** The last segment of a relative path, what a tab shows as its label. */
export function tabLabel(relativePath: string): string {
  const parts = relativePath.split(/[\\/]/).filter((part) => part !== "");
  return parts[parts.length - 1] ?? relativePath;
}

/**
 * Opens a file: a new tab at the end, which becomes the active one. A file that already has a tab
 * activates it instead; its record is replaced only when the content is another (a different
 * hash, e.g. after an Analyse), so the same file is never listed twice.
 */
export function openTab(state: TabsState, folder: ViewerFolder, record: FileRecord): TabsState {
  const id = tabId(folder, record.relativePath);
  const existing = state.tabs.findIndex((tab) => tab.id === id);
  if (existing === -1) {
    return { tabs: [...state.tabs, { id, folder, record }], activeId: id };
  }
  const tabs =
    state.tabs[existing]?.record.sha256 === record.sha256
      ? state.tabs
      : state.tabs.map((tab, index) => (index === existing ? { ...tab, record } : tab));
  return { tabs, activeId: id };
}

/** Shows another tab. An id that is not open changes nothing. */
export function activateTab(state: TabsState, id: string): TabsState {
  return state.activeId === id || !state.tabs.some((tab) => tab.id === id)
    ? state
    : { ...state, activeId: id };
}

/**
 * Closes tabs. When the active one goes, its right neighbour (among those that stay) takes over,
 * else the left one. Closing the last tab gives the empty state.
 */
export function closeTabs(state: TabsState, ids: readonly string[]): TabsState {
  const gone = new Set(ids);
  if (!state.tabs.some((tab) => gone.has(tab.id))) {
    return state;
  }
  const tabs = state.tabs.filter((tab) => !gone.has(tab.id));
  if (tabs.length === 0) {
    return NO_TABS;
  }
  if (state.activeId !== null && !gone.has(state.activeId)) {
    return { tabs, activeId: state.activeId };
  }
  const position = state.tabs.findIndex((tab) => tab.id === state.activeId);
  const right = state.tabs.slice(position + 1).find((tab) => !gone.has(tab.id));
  const left = state.tabs
    .slice(0, Math.max(position, 0))
    .reverse()
    .find((tab) => !gone.has(tab.id));
  return { tabs, activeId: (right ?? left ?? tabs[0])?.id ?? null };
}

export function closeTab(state: TabsState, id: string): TabsState {
  return closeTabs(state, [id]);
}

/** The ids of the tabs that belong to one folder. */
export function idsOfFolder(state: TabsState, folder: ViewerFolder): string[] {
  return state.tabs.filter((tab) => tab.folder === folder).map((tab) => tab.id);
}

/** The tabs a view holds; the welcome view and a closed centre hold none. */
export function tabsOf(view: WorkspaceView | null): TabsState {
  return view?.kind === "files" ? { tabs: view.tabs, activeId: view.activeId } : NO_TABS;
}

/** The view for a list of tabs: `null` (centre closed) when no tab is left. */
export function viewOf(state: TabsState): WorkspaceView | null {
  return state.tabs.length === 0
    ? null
    : { kind: "files", tabs: [...state.tabs], activeId: state.activeId };
}
