import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * Guards for the rule that makes a rendered answer safe: markdown carries **formatting**, and
 * nothing else. Anything naming a file, a path or an action travels on the structured channel
 * beside the stream, where Rust re-validates it (`docs/CHAT-UX-ASSESSMENT.md`, item 1b).
 *
 * The reason is recorded history rather than caution: `docs/DECISIONS.md` keeps the trial where a
 * prompt made the model invent `rename_file` and `delete_calendar_event` calls, and the conclusion
 * that file actions live in code rather than in model output. An answer is written from extracted
 * document text, so a poisoned PDF choosing the target of a link is the same failure in a new
 * costume. These read the sources, because that is the kind of rule a well-meaning commit relaxes
 * by accident.
 */

const HERE = fileURLToPath(new URL(".", import.meta.url));
const CLIENT_SOURCE = resolve(HERE, "..");
const RENDERER = join(CLIENT_SOURCE, "components", "Markdown.tsx");
const MANIFEST = resolve(HERE, "../../package.json");

function componentsUnder(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return componentsUnder(path);
    }
    return entry.name.endsWith(".tsx") ? [path] : [];
  });
}

describe("model output never becomes markup", () => {
  const components = componentsUnder(CLIENT_SOURCE);

  it("finds components to check", () => {
    expect(components.length).toBeGreaterThan(5);
  });

  it.each(components)("%s does not set HTML itself", (path) => {
    expect(readFileSync(path, "utf8")).not.toMatch(/dangerouslySetInnerHTML/);
  });

  it("does not let raw HTML through the markdown pipeline", () => {
    const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
      dependencies: Record<string, string>;
      devDependencies: Record<string, string>;
    };
    const declared = Object.keys({ ...manifest.dependencies, ...manifest.devDependencies });

    // `react-markdown` ignores embedded HTML unless this plugin is added, which is why none of the
    // answer can become an element the model chose.
    expect(declared).not.toContain("rehype-raw");
  });
});

describe("a link in an answer is text, not a destination", () => {
  // What that produces is tested for real in `src/components/markdown.test.ts`. This only holds
  // the overrides in place, because removing them is a one-line change that looks like a cleanup.
  const source = readFileSync(RENDERER, "utf8");

  it("replaces the anchor and the image with their own text", () => {
    expect(source).toMatch(/\ba:\s*\(/);
    expect(source).toMatch(/\bimg:\s*\(/);
  });
});
