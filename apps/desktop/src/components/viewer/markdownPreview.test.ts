import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { prepareViewerText, TEXT_VIEW_LIMIT } from "../../lib/viewerText";
import { Markdown } from "../Markdown";

/** What the Preview draws for a file's bytes: the same chain as `MarkdownPreview`, without a window. */
function previewOf(source: string | Uint8Array): string {
  const bytes = typeof source === "string" ? new TextEncoder().encode(source) : source;
  return renderToStaticMarkup(createElement(Markdown, { text: prepareViewerText(bytes).text }));
}

describe("the Markdown preview", () => {
  it("shows a link as its text, with no anchor and no destination", () => {
    const html = previewOf("Voir [le site](https://example.invalid) ici.");

    expect(html).toBe("<p>Voir le site ici.</p>");
    expect(html).not.toContain("<a");
    expect(html).not.toContain("example.invalid");
  });

  it("shows an image as its alt text, with no element that could fetch it", () => {
    const html = previewOf("![schema du circuit](https://example.invalid/x.png)");

    expect(html).toContain("schema du circuit");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("example.invalid");
  });

  it("does not link a reference-style link, an autolink or a file path either", () => {
    const html = previewOf(
      "[ref][1] <https://example.invalid/auto> [chemin](file:///C:/secret.txt)\n\n[1]: https://example.invalid/ref",
    );

    expect(html).not.toContain("<a");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("href");
    expect(html).not.toContain("file:///");
  });

  it("renders a table, a code fence and a quote without failing", () => {
    const html = previewOf(
      "| Nom | Valeur |\n| --- | --- |\n| a | 1 |\n\n```\ncode\n```\n\n> citation\n\n- [ ] a faire\n- [x] fait",
    );

    expect(html).toContain("<table>");
    expect(html).toContain("<pre><code>code\n</code></pre>");
    expect(html).toContain("<blockquote>");
    expect(html).toContain('type="checkbox"');
  });

  it("keeps the accents of the decoded bytes", () => {
    const html = previewOf("# Resume\n\nPatiente ecrite en francais: \u00e9\u00e8\u00e0\u00e7 \u0153 \u20ac");

    expect(html).toContain("\u00e9\u00e8\u00e0\u00e7 \u0153 \u20ac");
  });

  it("does not turn markup in the file into markup", () => {
    const html = previewOf("<script>alert(1)</script> <img src=x onerror=alert(1)>");

    expect(html).not.toContain("<script");
    expect(html).not.toContain("<img");
  });

  it("is given the same bounded text as the raw view", () => {
    const big = new TextEncoder().encode("ligne de texte\n".repeat(TEXT_VIEW_LIMIT));
    const prepared = prepareViewerText(big);

    expect(prepared.truncated).toBe(true);
    expect(prepared.text.length).toBeLessThanOrEqual(TEXT_VIEW_LIMIT);
  });
});
