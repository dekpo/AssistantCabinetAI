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

describe("groupSources, sections", () => {
  const withSection = (relativePath: string, pageNumber: number, title: string | null): Evidence =>
    ({
      relativePath,
      pageNumber,
      origin: "textLayer",
      section: title === null ? null : { title, level: 1, firstPage: 1, lastPage: 9 },
    }) as Evidence;

  it("names the sections the excerpts start in, once each, in order", () => {
    const [group] = groupSources([
      withSection("a.pdf", 3, "2.1 Hygiène"),
      withSection("a.pdf", 5, "2.1 Hygiène"),
      withSection("a.pdf", 8, "3 Déchets"),
    ]);

    expect(group?.sections).toStrictEqual(["2.1 Hygiène", "3 Déchets"]);
  });

  it("names nothing for a document with no structure or a slice of pages", () => {
    const [group] = groupSources([withSection("a.pdf", 3, null), withSection("a.pdf", 4, "")]);

    expect(group?.sections).toStrictEqual([]);
  });
});
