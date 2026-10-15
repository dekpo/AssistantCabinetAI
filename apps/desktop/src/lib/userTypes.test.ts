import { describe, expect, it } from "vitest";
import { codeFromLabel, phrasesFromText, textFromPhrases, userTypeFromForm } from "./userTypes";

describe("codeFromLabel", () => {
  it("makes a code from the name: the prefix, lower case, no accent, underscores", () => {
    expect(codeFromLabel("Bail d'habitation")).toBe("user_bail_d_habitation");
    expect(codeFromLabel("  Relevé  CSS ")).toBe("user_releve_css");
  });

  it("has no code for a name with nothing to build one from", () => {
    expect(codeFromLabel("   ")).toBeNull();
    expect(codeFromLabel("???")).toBeNull();
  });

  it("stays within the longest code", () => {
    const code = codeFromLabel("a".repeat(100));
    expect(code?.length).toBeLessThanOrEqual(48);
  });
});

describe("phrases", () => {
  it("reads one phrase a line, or separated by semicolons, without blanks or repeats", () => {
    expect(phrasesFromText("bail\n loyer ;  bail \n\n")).toEqual(["bail", "loyer"]);
  });

  it("writes them back one a line", () => {
    expect(textFromPhrases(["bail", "loyer"])).toBe("bail\nloyer");
  });
});

describe("userTypeFromForm", () => {
  it("builds the rule the index stores", () => {
    const type = userTypeFromForm({
      label: "Bail",
      scope: "documents",
      name: "bail",
      title: "contrat de bail",
      header: "",
      body: "loyer\ncharges",
      sheet: "",
    });
    expect(type).toEqual({
      code: "user_bail",
      scope: "documents",
      label: "Bail",
      name: ["bail"],
      title: ["contrat de bail"],
      header: [],
      body: ["loyer", "charges"],
      sheet: [],
    });
  });

  it("refuses a rule with no phrase at all, or no name", () => {
    expect(
      userTypeFromForm({ label: "Bail", scope: "documents", name: "", title: "", header: "", body: "", sheet: "" }),
    ).toBeNull();
    expect(
      userTypeFromForm({ label: " ", scope: "documents", name: "bail", title: "", header: "", body: "", sheet: "" }),
    ).toBeNull();
  });
});
