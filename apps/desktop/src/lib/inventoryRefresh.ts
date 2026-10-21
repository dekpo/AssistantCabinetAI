/*
 * When the explorer reads its two folder inventories again. Since explorer step 8 the state of a file of
 * the other family lives in the **other** folder's inventory (a workbook of the Documents folder is in the
 * Documents inventory, joined to what the tabular pass made of it), so a pass of one family changes what
 * both inventories say. Both are therefore read again after any pass, and after the analysis of either
 * family is cleared, without waiting for the window to lose and regain the focus.
 */

export interface RefreshInput {
  /** Counted up by every finished pass of each family. */
  documentsPasses: number;
  dataPasses: number;
  /** Counted up when the software itself wrote a file into the Documents folder (a saved note). */
  notesToken: number;
  /** Counted up when the analysis of a family was cleared. */
  documentsClears: number;
  tablesClears: number;
}

export interface InventoryRefresh {
  /** The finished-pass counter given to both inventories. */
  passes: number;
  /** The refresh token given to both inventories. */
  token: number;
}

export function inventoryRefresh(input: RefreshInput): InventoryRefresh {
  return {
    passes: input.documentsPasses + input.dataPasses,
    token: input.notesToken + input.documentsClears + input.tablesClears,
  };
}
