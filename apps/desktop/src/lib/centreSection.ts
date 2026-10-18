import type { WorkspaceView } from "./workspaceView";

/** The two sections of the centre zone, chosen by the Files | Notes switch. */
export type CentreSection = "files" | "notes";

const EMPTY_FILES: WorkspaceView = { kind: "welcome" };

/** The section on screen: Notes can only be shown while its own switch is on. */
export function effectiveSection(section: CentreSection, notesEnabled: boolean): CentreSection {
  return notesEnabled ? section : "files";
}

/**
 * What `AppLayout` is given: the view that makes the centre zone open, or `null` for a closed zone.
 * `AppLayout` opens the zone exactly while this is not `null`, so the open file tabs alone cannot
 * decide it any more: the zone also stays open while the Notes section is shown, or while it holds
 * a note that is not saved, even after the last file tab was closed. With the Notes switch off this
 * is the file tabs' own view, as before.
 */
export function shownCentre(input: {
  /** The open file tabs (or the welcome view), or `null`. */
  workspace: WorkspaceView | null;
  /** The user hid the zone (button or drag): the tabs and the drafts stay, the zone is closed. */
  hidden: boolean;
  notesEnabled: boolean;
  section: CentreSection;
  /** The Notes section holds a note with text, or a recording. */
  notesHold: boolean;
}): WorkspaceView | null {
  if (input.hidden) {
    return null;
  }
  if (input.workspace !== null) {
    return input.workspace;
  }
  if (!input.notesEnabled) {
    return null;
  }
  return input.section === "notes" || input.notesHold ? EMPTY_FILES : null;
}
