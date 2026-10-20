/*
 * Where the « ⋯ » menu of a family opens. The menu is placed in fixed coordinates, because the box
 * clips whatever overflows it and a short box (both families folded) would cut a menu hanging below
 * its last line. Pure, so the arithmetic is tested without a window (`explorerMenu.test.ts`).
 */

/** Height of one menu entry and of the menu's own padding, in pixels (`.explorer-menu` in the styles). */
export const MENU_ITEM_PX = 30;
export const MENU_PADDING_PX = 8;
/** The gap between the button and the menu. */
const GAP_PX = 2;

export interface MenuRect {
  top: number;
  bottom: number;
  right: number;
}

export interface MenuPlacement {
  top: number;
  /** Distance from the right edge of the window: the menu is right-aligned with its button. */
  right: number;
}

export function menuHeight(items: number): number {
  return items * MENU_ITEM_PX + MENU_PADDING_PX;
}

/** Under the button, or above it when there is no room below and there is above. */
export function menuPlacement(
  button: MenuRect,
  viewport: { width: number; height: number },
  items: number,
): MenuPlacement {
  const height = menuHeight(items);
  const right = Math.max(0, viewport.width - button.right);
  const below = button.bottom + GAP_PX;
  if (below + height <= viewport.height || button.top - GAP_PX - height < 0) {
    return { top: below, right };
  }
  return { top: button.top - GAP_PX - height, right };
}

/** The entry to focus after an arrow key, Home or End: it wraps, and an empty menu focuses nothing. */
export function nextMenuIndex(current: number, count: number, key: string): number | null {
  if (count === 0) {
    return null;
  }
  switch (key) {
    case "ArrowDown":
      return (current + 1) % count;
    case "ArrowUp":
      return (current - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}
