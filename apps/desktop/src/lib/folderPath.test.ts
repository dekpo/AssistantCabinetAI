import { describe, expect, it } from "vitest";
import { abbreviateFolderPath } from "./folderPath";

describe("abbreviateFolderPath", () => {
  it("keeps only the last folder of a Windows path, with its own separator", () => {
    expect(abbreviateFolderPath("C:\\Users\\practice\\AssistantCabinetAI\\DOCS")).toBe(
      "…\\DOCS",
    );
  });

  it("keeps only the last folder of a macOS path, with its own separator", () => {
    expect(abbreviateFolderPath("/Users/practice/AssistantCabinetAI/DOCS")).toBe("…/DOCS");
  });

  it("drops a trailing separator rather than showing an empty name", () => {
    expect(abbreviateFolderPath("C:\\Users\\practice\\AssistantCabinetAI\\DOCS\\")).toBe(
      "…\\DOCS",
    );
  });

  it("leaves a path with nothing to shorten as it is", () => {
    expect(abbreviateFolderPath("")).toBe("");
  });

  it("works the same for the Data Folder as for the Documents Folder", () => {
    expect(abbreviateFolderPath("C:\\Users\\practice\\AssistantCabinetAI\\DATA")).toBe(
      "…\\DATA",
    );
  });
});
