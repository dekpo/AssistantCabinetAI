import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Markdown } from "./Markdown";

/**
 * What the renderer actually produces. No browser is needed for this: rendering to static markup
 * runs in Node, which is why these are behaviour rather than another source-reading guard.
 */
function render(text: string): string {
  return renderToStaticMarkup(createElement(Markdown, { text }));
}

describe("an answer reads as formatting", () => {
  it("emphasises rather than showing the asterisks", () => {
    expect(render("**Patiente** et *antecedents*")).toBe(
      "<p><strong>Patiente</strong> et <em>antecedents</em></p>",
    );
  });

  it("builds a list out of dashes", () => {
    const html = render("- premier\n- second");

    expect(html).toContain("<ul>");
    expect(html).toContain("<li>premier</li>");
  });

  it("builds a heading, kept modest by the stylesheet", () => {
    expect(render("### Synthese")).toBe("<h3>Synthese</h3>");
  });

  it("builds a table, which is how a summary of results comes back", () => {
    const html = render("| Analyse | Valeur |\n| --- | --- |\n| HbA1c | 7,2 % |");

    expect(html).toContain("<table>");
    expect(html).toContain("<th>Analyse</th>");
    expect(html).toContain("<td>HbA1c</td>");
  });

  it("keeps a line break she can see the reason for", () => {
    expect(render("premier paragraphe\n\nsecond paragraphe")).toBe(
      "<p>premier paragraphe</p>\n<p>second paragraphe</p>",
    );
  });
});

/**
 * The rest of this file is the security property, not a nicety. An answer is written from
 * extracted document text, so everything below is what a poisoned document would try
 * (`docs/CHAT-UX-ASSESSMENT.md`, item 1b).
 */
describe("an answer carries formatting and nothing else", () => {
  it("renders a link as its own text, with no destination", () => {
    const html = render("Voir [le compte-rendu](file:///C:/Users/practice/anywhere.pdf).");

    expect(html).toBe("<p>Voir le compte-rendu.</p>");
    expect(html).not.toContain("file:///");
  });

  it("renders an image as its description, so nothing is fetched", () => {
    const html = render("![tableau de resultats](http://elsewhere.example/pixel.png)");

    expect(html).toContain("tableau de resultats");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("elsewhere.example");
  });

  it("does not turn a bare address into a link either", () => {
    const html = render("Voir http://elsewhere.example/page pour la suite.");

    expect(html).not.toContain("<a");
    expect(html).toContain("http://elsewhere.example/page");
  });

  it("shows markup the model wrote instead of applying it", () => {
    const html = render("<script>alert(1)</script>");

    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("shows an inline element the model wrote instead of applying it", () => {
    const html = render("Bonjour <img src=x onerror=alert(1)> madame");

    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
  });
});

describe("a half-written answer is still readable", () => {
  // Every delta re-renders the answer, so the parser meets syntactically incomplete markdown
  // several times a second and must not throw on any of it.
  it.each(["**Pati", "- premier\n-", "| Analyse |", "###", "["])(
    "renders %j without failing",
    (partial) => {
      expect(() => render(partial)).not.toThrow();
    },
  );

  it("shows nothing at all for an answer that has not started", () => {
    expect(render("")).toBe("");
  });
});
