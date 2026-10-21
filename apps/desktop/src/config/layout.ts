/*
 * The numbers that size the lists of the interface, in one place. Change one here and the
 * application takes it at the next start (`pnpm tauri dev` takes it at once). None of them is a
 * setting of the user: they are the choices of the design.
 */

/** Rows of a file list shown at once in the Settings panel; the rest scrolls inside the list. */
export const SETTINGS_LIST_ROWS = 25;

/** Rows shared by the open lists of the sidebar's box: both open, this many each. */
export const ROWS_BOTH_OPEN = 6;

/** One open list of the sidebar's box takes this many rows. */
export const ROWS_ONE_OPEN = 12;
