/*
 * Which situation the explorer box is in, decided from what the two folders hold and from the
 * settings, with no React and no call (`explorerStates.test.ts`). The box draws one of three
 * situations (`docs/DECISIONS.md`, "First start", "Folders created, still empty"): nothing chosen
 * yet, folders chosen but empty, and the box in use. A family can be in a different situation from
 * the other one (one folder chosen, the other not yet), so each family has its own.
 */

/** What one family's folder is: not chosen, chosen and holding nothing, or chosen and (possibly)
 * holding files. A folder whose inventory is not read yet counts as the last: the box must not flash
 * an "empty" message for a folder that is simply being read. */
export type FolderState = "none" | "empty" | "filled";

/** The whole box: nothing chosen on either side, nothing to list on either side, or in use. */
export type BoxState = "firstStart" | "empty" | "use";

export function folderState(input: {
  /** The folder of the family, or null while none is chosen. */
  folder: string | null;
  /** The inventory of that folder was read (a failed or pending read is not an empty folder). */
  loaded: boolean;
  /** Every file the inventory reports in that folder, readable by the product or not. */
  folderFiles: number;
  /** The files listed under this family, wherever they sit (a file of the other folder counts). */
  familyFiles: number;
}): FolderState {
  if (input.folder === null) {
    return "none";
  }
  return input.loaded && input.folderFiles === 0 && input.familyFiles === 0 ? "empty" : "filled";
}

export function boxState(documents: FolderState, tables: FolderState): BoxState {
  if (documents === "none" && tables === "none") {
    return "firstStart";
  }
  return documents === "filled" || tables === "filled" ? "use" : "empty";
}

/** Search and filters have nothing to look through until a folder holds a file. */
export function nothingToSearch(box: BoxState): boolean {
  return box !== "use";
}

/** What "Créer les deux dossiers suggérés" made of each family: a path, or the failure that left
 * that family alone. One failing never loses the other. */
export interface SuggestedOutcome<Failure> {
  path: string | null;
  failure: Failure | null;
}

export async function attemptFolder<Failure>(
  make: () => Promise<string>,
  toFailure: (raw: unknown) => Failure,
): Promise<SuggestedOutcome<Failure>> {
  try {
    return { path: await make(), failure: null };
  } catch (raw: unknown) {
    return { path: null, failure: toFailure(raw) };
  }
}
