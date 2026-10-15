import { describe, expect, it } from "vitest";
import { foundPassages, snippetOf } from "./foundPassages";
import type { Evidence } from "./ipc";

function passage(over: Partial<Evidence>): Evidence {
  return {
    chunkId: "c1",
    relativePath: "a.pdf",
    pageNumber: 1,
    ordinal: 0,
    lastPage: 1,
    section: null,
    matches: [],
    text: "texte",
    score: 1,
    origin: "text_layer",
    confidence: null,
    ...over,
  } as Evidence;
}

describe("snippetOf", () => {
  it("marks the words of the question and cuts the rest around them", () => {
    const text = `${"a".repeat(300)} la franchise annuelle est de 300 francs ${"b".repeat(300)}`;
    const start = text.indexOf("franchise");
    const parts = snippetOf(text, [{ start, end: start + "franchise".length }], 40);
    expect(parts.filter((part) => part.marked).map((part) => part.text)).toEqual(["franchise"]);
    const joined = parts.map((part) => part.text).join("");
    expect(joined.startsWith("…")).toBe(true);
    expect(joined.endsWith("…")).toBe(true);
    expect(joined.length).toBeLessThan(120);
  });

  it("counts offsets in characters, not in UTF-16 units", () => {
    const text = "\u{1F600} la franchise est annuelle";
    const start = Array.from(text).indexOf("f");
    const parts = snippetOf(text, [{ start, end: start + 9 }], 40);
    expect(parts.find((part) => part.marked)?.text).toBe("franchise");
  });

  it("shows the beginning of a passage found by meaning alone, with nothing marked", () => {
    const parts = snippetOf("x ".repeat(200), [], 40);
    expect(parts.some((part) => part.marked)).toBe(false);
    expect(parts.map((part) => part.text).join("").endsWith("…")).toBe(true);
  });
});

describe("foundPassages", () => {
  const word = { start: 0, end: 4 };

  it("puts the files with a word of the question first and the others apart", () => {
    const found = foundPassages([
      passage({ chunkId: "1", relativePath: "meaning.pdf" }),
      passage({ chunkId: "2", relativePath: "words.pdf", matches: [word] }),
      passage({ chunkId: "3", relativePath: "words.pdf", pageNumber: 4, lastPage: 5, matches: [word] }),
    ]);
    expect(found.matching.map((file) => file.relativePath)).toEqual(["words.pdf"]);
    expect(found.byMeaning.map((file) => file.relativePath)).toEqual(["meaning.pdf"]);
    expect(found.passageCount).toBe(3);
    expect(found.documentCount).toBe(2);
    const pages = found.matching[0]!.passages.map((item) => [item.firstPage, item.lastPage]);
    expect(pages).toEqual([
      [1, 1],
      [4, 5],
    ]);
  });

  it("keeps a passage without a word inside a file that has other words, flagged as found by meaning", () => {
    const found = foundPassages([
      passage({ chunkId: "1", relativePath: "mixed.pdf", matches: [word] }),
      passage({ chunkId: "2", relativePath: "mixed.pdf", pageNumber: 9, lastPage: 9 }),
    ]);
    expect(found.byMeaning).toEqual([]);
    expect(found.matching[0]!.passages.map((item) => item.byMeaning)).toEqual([false, true]);
  });

  it("carries the section title when the document has a structure", () => {
    const found = foundPassages([
      passage({ matches: [word], section: { title: "Franchise" } as Evidence["section"] }),
    ]);
    expect(found.matching[0]!.passages[0]!.section).toBe("Franchise");
  });

  it("is empty without sources", () => {
    expect(foundPassages([])).toEqual({ matching: [], byMeaning: [], passageCount: 0, documentCount: 0 });
  });
});
