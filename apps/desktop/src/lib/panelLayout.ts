import type { PanelLayouts, PanelTriple } from "./ipc";

/**
 * The widths of the three zones, between the shape `react-resizable-panels` hands over (an object
 * keyed by panel id, in percent) and the shape the settings store (a triple, see `PanelLayouts`).
 * Pure, so what cannot be seen in a unit test (the dragging) at least rests on checked arithmetic.
 */

export const RESOURCES_ID = "resources";
export const WORKSPACE_ID = "workspace";
export const CHAT_ID = "chat";

/** The library's own shape: panel id to percentage. */
export type PanelLayout = Record<string, number>;

/** The proportions before anything is saved, and after a restore. */
export const DEFAULT_OPEN_TRIPLE: PanelTriple = [22, 38, 40];
export const DEFAULT_COLLAPSED_TRIPLE: PanelTriple = [26, 0, 74];

export const NO_LAYOUTS: PanelLayouts = { open: null, collapsed: null };

/** Below this share (percent) the centre zone counts as collapsed: the library reports a collapsed
 * panel as its collapsed size, and a float can leave a hair of it. */
const COLLAPSED_BELOW = 0.1;

export function layoutToTriple(layout: PanelLayout): PanelTriple | null {
  const triple: PanelTriple = [layout[RESOURCES_ID], layout[WORKSPACE_ID], layout[CHAT_ID]] as PanelTriple;
  const usable = triple.every(
    (share) => typeof share === "number" && Number.isFinite(share) && share >= 0 && share <= 100,
  );
  return usable ? triple : null;
}

export function tripleToLayout(triple: PanelTriple): PanelLayout {
  return { [RESOURCES_ID]: triple[0], [WORKSPACE_ID]: triple[1], [CHAT_ID]: triple[2] };
}

/** Whether the centre zone takes room in this layout. */
export function isCentreOpen(layout: PanelLayout): boolean {
  const share = layout[WORKSPACE_ID];
  return typeof share === "number" && Number.isFinite(share) && share > COLLAPSED_BELOW;
}

/**
 * The saved triple that applies when the centre is shown (`open`) or not. `null` when nothing
 * usable is saved: the caller then takes the default for that state.
 */
export function savedTriple(layouts: PanelLayouts, centreOpen: boolean): PanelTriple | null {
  const triple = centreOpen ? layouts.open : layouts.collapsed;
  if (triple === null || triple === undefined) {
    return null;
  }
  const usable =
    triple.length === 3 &&
    triple.every((share) => Number.isFinite(share) && share >= 0 && share <= 100) &&
    (centreOpen ? triple[1] > 0 : triple[1] === 0);
  return usable ? triple : null;
}

/** The layout to put on screen for a state of the centre: the saved proportions, else the default. */
export function layoutToApply(layouts: PanelLayouts, centreOpen: boolean): PanelLayout {
  const fallback = centreOpen ? DEFAULT_OPEN_TRIPLE : DEFAULT_COLLAPSED_TRIPLE;
  return tripleToLayout(savedTriple(layouts, centreOpen) ?? fallback);
}

/**
 * What to store after a layout the user just made: the layouts with the matching triple replaced.
 * A collapsed centre is stored as exactly zero (the settings require it), the other two shares
 * unchanged. `null` when the library's layout is unusable, so nothing is stored.
 */
export function storeLayout(layouts: PanelLayouts, layout: PanelLayout): PanelLayouts | null {
  const triple = layoutToTriple(layout);
  if (triple === null) {
    return null;
  }
  if (isCentreOpen(layout)) {
    return { ...layouts, open: triple };
  }
  return { ...layouts, collapsed: [triple[0], 0, triple[2]] };
}

export function sameLayouts(a: PanelLayouts, b: PanelLayouts): boolean {
  const same = (x: PanelTriple | null, y: PanelTriple | null) =>
    x === y || (x !== null && y !== null && x.every((share, index) => share === y[index]));
  return same(a.open, b.open) && same(a.collapsed, b.collapsed);
}
