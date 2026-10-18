import { describe, expect, it } from "vitest";
import { effectiveSection, shownCentre } from "./centreSection";
import type { WorkspaceView } from "./workspaceView";

const FILES: WorkspaceView = { kind: "files", tabs: [], activeId: null };
const WELCOME: WorkspaceView = { kind: "welcome" };

const base = {
  workspace: null as WorkspaceView | null,
  hidden: false,
  notesEnabled: true,
  section: "files" as const,
  notesHold: false,
};

describe("what the layout is given for the centre zone", () => {
  it("is the file tabs' own view while the Notes switch is off, hidden or not", () => {
    expect(shownCentre({ ...base, notesEnabled: false, workspace: FILES })).toBe(FILES);
    expect(shownCentre({ ...base, notesEnabled: false, workspace: FILES, hidden: true })).toBeNull();
    expect(shownCentre({ ...base, notesEnabled: false, section: "notes", notesHold: true })).toBeNull();
  });

  it("is nothing at launch: no tab, no note, files shown", () => {
    expect(shownCentre(base)).toBeNull();
  });

  it("opens on the Notes section even with no file tab", () => {
    expect(shownCentre({ ...base, section: "notes" })).toStrictEqual(WELCOME);
  });

  it("stays open on a note with text after the last file tab was closed", () => {
    expect(shownCentre({ ...base, notesHold: true })).toStrictEqual(WELCOME);
  });

  it("keeps the file tabs' view when there is one", () => {
    expect(shownCentre({ ...base, workspace: FILES, section: "notes", notesHold: true })).toBe(FILES);
  });

  it("is closed whenever the user hid the zone, notes or not", () => {
    expect(shownCentre({ ...base, hidden: true, section: "notes", notesHold: true })).toBeNull();
    expect(shownCentre({ ...base, hidden: true, workspace: FILES })).toBeNull();
  });
});

describe("the section on screen", () => {
  it("is Files whenever the Notes switch is off", () => {
    expect(effectiveSection("notes", false)).toBe("files");
    expect(effectiveSection("notes", true)).toBe("notes");
    expect(effectiveSection("files", true)).toBe("files");
  });
});
