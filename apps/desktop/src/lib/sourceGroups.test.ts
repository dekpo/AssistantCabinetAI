import { describe, expect, it } from "vitest";
import type { Evidence } from "./ipc";
import { groupSources } from "./sourceGroups";

function excerpt(relativePath: string, pageNumber: number, origin = "text_layer"): Evidence {
  return { relativePath, pageNumber, origin } as Evidence;
}

describe("groupSources", () => {
  it("lists a file once however many excerpts came from it", () => {
    const groups = groupSources([
      excerpt("absence.pdf", 1),
      excerpt("ordonnance.pdf", 1),
      excerpt("ordonnance.pdf", 1),
    ]);

    expect(groups.map((group) => group.relativePath)).toStrictEqual([
      "absence.pdf",
      "ordonnance.pdf",
    ]);
    expect(groups[1]?.pages).toStrictEqual([1]);
  });

  it("collects the pages of a file in ascending order", () => {
    const [group] = groupSources([excerpt("a.pdf", 3), excerpt("a.pdf", 1), excerpt("a.pdf", 3)]);

    expect(group?.pages).toStrictEqual([1, 3]);
  });

  it("keeps the order in which files first appear", () => {
    const groups = groupSources([excerpt("b.pdf", 1), excerpt("a.pdf", 1), excerpt("b.pdf", 2)]);

    expect(groups.map((group) => group.relativePath)).toStrictEqual(["b.pdf", "a.pdf"]);
  });

  it("marks a file read by recognition when any of its excerpts was", () => {
    const [group] = groupSources([excerpt("a.pdf", 1), excerpt("a.pdf", 2, "ocr")]);

    expect(group?.ocr).toBe(true);
  });
});
