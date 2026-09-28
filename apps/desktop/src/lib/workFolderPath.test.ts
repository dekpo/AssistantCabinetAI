import { describe, expect, it } from "vitest";
import { abbreviateWorkFolderPath } from "./workFolderPath";

describe("abbreviateWorkFolderPath", () => {
  it("keeps only the last folder of a Windows path, with its own separator", () => {
    expect(abbreviateWorkFolderPath("C:\\Users\\practice\\AssistantCabinetAI\\DOCS")).toBe(
      "…\\DOCS",
    );
  });

  it("keeps only the last folder of a macOS path, with its own separator", () => {
    expect(abbreviateWorkFolderPath("/Users/practice/AssistantCabinetAI/DOCS")).toBe("…/DOCS");
  });

  it("drops a trailing separator rather than showing an empty name", () => {
    expect(abbreviateWorkFolderPath("C:\\Users\\practice\\AssistantCabinetAI\\DOCS\\")).toBe(
      "…\\DOCS",
    );
  });

  it("leaves a path with nothing to shorten as it is", () => {
    expect(abbreviateWorkFolderPath("")).toBe("");
  });
});
