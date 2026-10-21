import { useMemo } from "react";
import { inventoryRefresh } from "../lib/inventoryRefresh";
import { mergeInventories, type MergedInventories } from "../lib/explorerModel";
import {
  dataFolderInventory,
  workFolderInventory,
  type DataFolderReport,
  type InventoryReport,
} from "../lib/ipc";
import { useFolderInventory } from "./useFolderInventory";

/**
 * The two folder inventories and the files of both joined under their family, read the same way
 * wherever the families are drawn: the box of the sidebar and the folder sections of the Settings
 * panel. A pass of one family changes what both inventories say (a workbook of the Documents folder is
 * read by the tabular pass), so both are read again after any pass or clear (`lib/inventoryRefresh.ts`).
 */
export function useFamilyInventories({
  workFolder,
  dataFolder,
  documentsPasses,
  dataPasses,
  notesToken,
  documentsClears,
  tablesClears,
}: {
  workFolder: string | null;
  dataFolder: string | null;
  /** Counted up by every finished pass of each family. */
  documentsPasses: number;
  dataPasses: number;
  /** Counted up when the software itself wrote a file into the Documents folder (a saved note). */
  notesToken: number;
  /** Counted up when the analysis of a family was cleared. */
  documentsClears: number;
  tablesClears: number;
}): {
  documentsInventory: InventoryReport | null;
  dataInventory: DataFolderReport | null;
  merged: MergedInventories;
} {
  const refresh = inventoryRefresh({
    documentsPasses,
    dataPasses,
    notesToken,
    documentsClears,
    tablesClears,
  });
  const documentsInventory = useFolderInventory(
    workFolderInventory,
    workFolder,
    refresh.passes,
    refresh.token,
  );
  const dataInventory = useFolderInventory(
    dataFolderInventory,
    dataFolder,
    refresh.passes,
    refresh.token,
  );
  const merged = useMemo(
    () =>
      mergeInventories(documentsInventory?.files ?? [], dataInventory?.files ?? [], {
        documents: workFolder,
        data: dataFolder,
      }),
    [documentsInventory, dataInventory, workFolder, dataFolder],
  );
  return { documentsInventory, dataInventory, merged };
}
