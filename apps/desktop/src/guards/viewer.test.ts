import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * Guards for the rules of the document viewer that a well-meaning commit breaks by accident: the
 * webview never reads files itself, nothing in the sources can reach another host, the library is
 * imported in one place only, and none of its fixed-language labels is left on screen.
 */

const HERE = fileURLToPath(new URL(".", import.meta.url));
const CLIENT_SOURCE = resolve(HERE, "..");
const ADAPTER = join(CLIENT_SOURCE, "lib", "viewerEngine.ts");

function sourcesUnder(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return sourcesUnder(path);
    }
    const isSource = /\.(ts|tsx)$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name);
    return isSource ? [path] : [];
  });
}

function withoutComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/^\s*\/\/.*$/gm, " ");
}

const sources = sourcesUnder(CLIENT_SOURCE);

describe("the webview reads no file by itself", () => {
  it("finds sources to check", () => {
    expect(sources.length).toBeGreaterThan(20);
  });

  it.each(sources)("%s does not use the file system plugin", (path) => {
    expect(withoutComments(readFileSync(path, "utf8"))).not.toMatch(/plugin-fs|convertFileSrc/);
  });

  it("does not depend on the file system plugin", () => {
    const manifest = readFileSync(resolve(HERE, "../../package.json"), "utf8");
    expect(manifest).not.toContain("plugin-fs");
  });
});

describe("nothing in the sources can reach another host", () => {
  it.each(sources)("%s holds no external URL outside a comment", (path) => {
    const code = withoutComments(readFileSync(path, "utf8"));
    expect([...code.matchAll(/https?:\/\/[^\s"'`)]+/g)].map(([url]) => url)).toStrictEqual([]);
  });
});

describe("the library is behind one adapter", () => {
  it.each(sources.filter((path) => path !== ADAPTER))("%s does not import the library", (path) => {
    expect(withoutComments(readFileSync(path, "utf8"))).not.toMatch(/shrimp-doc-viewer/);
  });

  it("is imported by the adapter", () => {
    expect(readFileSync(ADAPTER, "utf8")).toContain('import("shrimp-doc-viewer")');
  });
});

describe("no fixed-language label of the library stays on screen", () => {
  it("hides the library toolbar, which holds the labels of one fixed language", () => {
    const styles = readFileSync(join(CLIENT_SOURCE, "styles.css"), "utf8");
    expect(styles).toMatch(
      /\.viewer__host > div:first-child:not\(\.dv-stage\)\s*\{\s*display:\s*none !important;/,
    );
  });

  it.each(sources)("%s writes no Chinese text of its own", (path) => {
    expect(readFileSync(path, "utf8")).not.toMatch(/\p{Script=Han}/u);
  });
});
