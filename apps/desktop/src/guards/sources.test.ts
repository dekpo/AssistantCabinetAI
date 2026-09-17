import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { lookupKey, type Catalogue } from "../i18n/translate";

/**
 * Guards for the language contract. They read sources rather than behaviour, because the rules
 * they protect are the ones a well-meaning commit breaks by accident: a French sentence in Rust,
 * a hardcoded label in a component, an error code no catalogue knows about.
 */

const HERE = fileURLToPath(new URL(".", import.meta.url));
const CLIENT_SOURCE = resolve(HERE, "..");
const RUST_SOURCE = resolve(HERE, "../../src-tauri/src");
const GATEWAY_ERRORS = resolve(
  HERE,
  "../../../server/src/assistant_cabinet_server/core/errors.py",
);

function filesUnder(directory: string, extension: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return filesUnder(path, extension);
    }
    return entry.name.endsWith(extension) ? [path] : [];
  });
}

function withoutComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/^\s*\/\/.*$/gm, " ");
}

const HAS_WORDS = /\p{L}{2,}/u;
/**
 * Text sitting between two tags on one line. Braces are excluded, so `>{t("key")}<` passes and
 * `>Work folder<` does not. A literal spread over several lines would slip through, which has not
 * happened yet and would still be caught in review.
 */
const TEXT_NODES = />([^<>{}\n]+)</g;
const VISIBLE_ATTRIBUTES = /\b(placeholder|title|aria-label|alt|label)="([^"]*)"/g;

describe("no user-visible literal in a component", () => {
  const components = filesUnder(CLIENT_SOURCE, ".tsx");

  it("finds components to check", () => {
    expect(components.length).toBeGreaterThan(5);
  });

  it.each(components)("%s puts its text through the catalogue", (path) => {
    const source = withoutComments(readFileSync(path, "utf8"));

    for (const [, text] of source.matchAll(TEXT_NODES)) {
      expect(HAS_WORDS.test(text ?? ""), `text node: ${text?.trim()}`).toBe(false);
    }
    for (const [, attribute, value] of source.matchAll(VISIBLE_ATTRIBUTES)) {
      expect(HAS_WORDS.test(value ?? ""), `${attribute}: ${value}`).toBe(false);
    }
  });
});

describe("no user language in the Rust sources", () => {
  const FRENCH_MARKERS =
    /\b(le|la|les|des|une|est|erreur|fichier|dossier|envoi|veuillez|aucune)\b/i;
  const rustFiles = filesUnder(RUST_SOURCE, ".rs");

  it("finds Rust sources to check", () => {
    expect(rustFiles.length).toBeGreaterThan(3);
  });

  it.each(rustFiles)("%s stays English and ASCII", (path) => {
    const source = readFileSync(path, "utf8");

    // An accented character is the cheapest sign that a sentence for a human slipped into code.
    expect([...source].filter((character) => character.charCodeAt(0) > 127)).toStrictEqual([]);
    for (const [index, line] of source.split("\n").entries()) {
      expect(FRENCH_MARKERS.test(line), `line ${index + 1}: ${line.trim()}`).toBe(false);
    }
  });
});

describe("every machine code has a sentence in every language", () => {
  function rustErrorCodes(): string[] {
    const source = readFileSync(join(RUST_SOURCE, "error.rs"), "utf8");
    return [...source.matchAll(/=>\s*"([a-z0-9_]+)"/g)].map(([, code]) => code as string);
  }

  function gatewayErrorCodes(): string[] {
    const source = readFileSync(GATEWAY_ERRORS, "utf8");
    return [...source.matchAll(/^\s+([a-z0-9_]+) = "([a-z0-9_]+)"$/gm)]
      .filter(([, name, value]) => name === value)
      .map(([, name]) => name as string);
  }

  const codes = [...new Set([...rustErrorCodes(), ...gatewayErrorCodes()])].sort();

  it("reads the codes from both sides", () => {
    expect(codes).toContain("work_folder_is_protected");
    expect(codes).toContain("model_alias_not_allowed");
  });

  it.each(SUPPORTED_LOCALES)("%s translates all of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const code of codes) {
      expect(lookupKey(catalogue, `errors.${code}`), code).toBeDefined();
    }
  });
});
