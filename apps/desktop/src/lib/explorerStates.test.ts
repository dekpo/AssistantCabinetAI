import { describe, expect, it } from "vitest";
import { attemptFolder, boxState, folderState, nothingToSearch } from "./explorerStates";

const chosen = { folder: "/docs", loaded: true, folderFiles: 0, familyFiles: 0 };

describe("the state of one family's folder", () => {
  it("is none while no folder is chosen, whatever the inventory says", () => {
    expect(folderState({ ...chosen, folder: null })).toBe("none");
    expect(folderState({ ...chosen, folder: null, folderFiles: 4, familyFiles: 4 })).toBe("none");
  });

  it("is empty when the folder is read and holds no file at all", () => {
    expect(folderState(chosen)).toBe("empty");
  });

  it("is never empty while the folder is still being read, or could not be read", () => {
    expect(folderState({ ...chosen, loaded: false })).toBe("filled");
  });

  it("is not empty when the folder holds a file the product does not read", () => {
    // A lone .ppt: nothing is listed, but the folder is not empty.
    expect(folderState({ ...chosen, folderFiles: 1, familyFiles: 0 })).toBe("filled");
  });

  it("is not empty when a file of the other folder waits under this family", () => {
    // Nothing in this folder, one document of the other folder listed under this family.
    expect(folderState({ ...chosen, folderFiles: 0, familyFiles: 1 })).toBe("filled");
  });

  it("is filled for a folder with files", () => {
    expect(folderState({ ...chosen, folderFiles: 12, familyFiles: 12 })).toBe("filled");
  });
});

describe("the state of the whole box", () => {
  it("is the first start only when neither folder is chosen", () => {
    expect(boxState("none", "none")).toBe("firstStart");
  });

  it("is empty when folders are chosen and none holds anything, one family or both", () => {
    expect(boxState("empty", "empty")).toBe("empty");
    expect(boxState("empty", "none")).toBe("empty");
    expect(boxState("none", "empty")).toBe("empty");
  });

  it("is in use as soon as one family holds a file, or may hold one", () => {
    expect(boxState("filled", "none")).toBe("use");
    expect(boxState("none", "filled")).toBe("use");
    expect(boxState("empty", "filled")).toBe("use");
    expect(boxState("filled", "filled")).toBe("use");
  });

  it("leaves search and filters off until a folder holds a file", () => {
    expect(nothingToSearch("firstStart")).toBe(true);
    expect(nothingToSearch("empty")).toBe(true);
    expect(nothingToSearch("use")).toBe(false);
  });
});

describe("making a suggested folder", () => {
  it("keeps the path when it is made and the failure when it is not", async () => {
    expect(await attemptFolder(async () => "/home/Docs", (raw) => String(raw))).toEqual({
      path: "/home/Docs",
      failure: null,
    });
    expect(
      await attemptFolder(
        async () => {
          throw "denied";
        },
        (raw) => `failed: ${String(raw)}`,
      ),
    ).toEqual({ path: null, failure: "failed: denied" });
  });
});
