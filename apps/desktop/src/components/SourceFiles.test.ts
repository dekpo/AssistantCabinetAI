import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import type { Evidence } from "../lib/ipc";
import { SourceFiles } from "./SourceFiles";

/* Drawn to static markup with the real catalogues, like `render.test.ts`: the words and the buttons the
   user would see under an answer, in the « Sources » disclosure. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale: "fr-FR", children: element }));
}

function passage(over: Partial<Evidence>): Evidence {
  return {
    chunkId: "c1",
    relativePath: "rapport.pdf",
    pageNumber: 2,
    ordinal: 0,
    lastPage: 2,
    section: null,
    text: "texte",
    score: 1,
    origin: "text_layer",
    confidence: null,
    ...over,
  } as Evidence;
}

describe("the documents listed under an answer", () => {
  it("offers « Résumer » and « Afficher le fichier » once for each document, with its pages", () => {
    const html = page(
      createElement(SourceFiles, {
        sources: [
          passage({ chunkId: "a", relativePath: "rapport.pdf", pageNumber: 2, lastPage: 2 }),
          passage({ chunkId: "b", relativePath: "rapport.pdf", pageNumber: 5, lastPage: 5 }),
          passage({ chunkId: "c", relativePath: "lettre.txt", pageNumber: 1, lastPage: 1 }),
        ],
      }),
    );
    expect(html.match(/Résumer/g)).toHaveLength(2);
    expect(html.match(/Afficher le fichier/g)).toHaveLength(2);
    expect(html).toContain("rapport.pdf");
    expect(html).toContain("lettre.txt");
  });

  it("shows no block of found passages and no marked word", () => {
    const html = page(createElement(SourceFiles, { sources: [passage({})] }));
    expect(html).not.toContain("Passages trouvés");
    expect(html).not.toContain("<mark");
    expect(html).not.toContain("found");
  });
});
