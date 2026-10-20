import { describe, expect, it } from "vitest";
import { effectiveSection, offeredSections, shownCentre, type CentreSection } from "./centreSection";
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

describe("the overview section", () => {
  it("opens the zone with no file tab and no note, with Notes on or off", () => {
    for (const notesEnabled of [true, false]) {
      expect(shownCentre({ ...base, notesEnabled, section: "overview" })).toStrictEqual(WELCOME);
    }
  });

  it("is closed when the user hid the zone, and never opens by itself", () => {
    expect(shownCentre({ ...base, section: "overview", hidden: true })).toBeNull();
    expect(shownCentre({ ...base, section: "overview", hidden: true, workspace: FILES })).toBeNull();
    // At launch the section is Files: nothing is open.
    expect(shownCentre({ ...base, notesEnabled: false })).toBeNull();
  });

  it("keeps the file tabs' view behind it, so the tabs come back as they were", () => {
    expect(shownCentre({ ...base, section: "overview", workspace: FILES })).toBe(FILES);
  });

  it("is a section whatever the Notes switch says", () => {
    expect(effectiveSection("overview", false)).toBe("overview");
    expect(effectiveSection("overview", true)).toBe("overview");
  });

  it("is offered first, Notes only while enabled", () => {
    expect(offeredSections(false)).toStrictEqual(["overview", "files"]);
    expect(offeredSections(true)).toStrictEqual(["overview", "files", "notes"]);
  });
});

describe("every combination of the centre's inputs", () => {
  const sections: CentreSection[] = ["overview", "files", "notes"];
  const flags = [true, false];

  it("is closed exactly when hidden, or when nothing asks for the zone", () => {
    for (const section of sections) {
      for (const notesEnabled of flags) {
        for (const hidden of flags) {
          for (const notesHold of flags) {
            for (const workspace of [null, FILES]) {
              const shown = shownCentre({ section, notesEnabled, hidden, notesHold, workspace });
              const asked =
                workspace !== null ||
                section === "overview" ||
                (notesEnabled && (section === "notes" || notesHold));
              expect(shown !== null, `${section} ${notesEnabled} ${hidden} ${notesHold} ${workspace === null}`).toBe(
                asked && !hidden,
              );
              if (shown !== null && workspace !== null) {
                expect(shown).toBe(workspace);
              }
            }
          }
        }
      }
    }
  });

  it("shows only a section on offer", () => {
    for (const section of sections) {
      for (const notesEnabled of flags) {
        expect(offeredSections(notesEnabled)).toContain(effectiveSection(section, notesEnabled));
      }
    }
  });
});
