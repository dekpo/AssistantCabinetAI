import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { COVERED_INTERNAL_NAMES, localiseInternalNames } from "./displayNames";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

describe("an internal name a model repeated", () => {
  it("is shown as the name the application uses, in her language", () => {
    expect(
      localiseInternalNames("Aucun document dans le WORK_FOLDER_CONTEXT ne mentionne ce prix.", french),
    ).toBe("Aucun document dans le Dossier des documents ne mentionne ce prix.");
    expect(localiseInternalNames("Nothing in the WORK_FOLDER_CONTEXT says so.", english)).toBe(
      "Nothing in the Documents folder says so.",
    );
  });

  it("is recognised in the variants a model writes", () => {
    for (const variant of ["WORK_FOLDER_CONTEXT", "WORK FOLDER CONTEXT", "work_folder_context", "Work Folder Context"]) {
      expect(localiseInternalNames(`See ${variant}.`, english)).toBe("See Documents folder.");
    }
    expect(localiseInternalNames("Table résultats : 1 450", french)).toBe("Dossier des données : 1 450");
    expect(localiseInternalNames("From the Table results block", english)).toBe("From the Data folder block");
    expect(localiseInternalNames("In the Document excerpts", english)).toBe("In the Documents folder");
  });

  it("shows the English word excerpt in her language, singular and plural, the capital kept", () => {
    expect(localiseInternalNames("Aucun document dans les excerpts fournis.", french)).toBe(
      "Aucun document dans les extraits fournis.",
    );
    expect(localiseInternalNames("Pas de prix dans l'excerpt.", french)).toBe("Pas de prix dans l'extrait.");
    expect(localiseInternalNames("Excerpts: none.", french)).toBe("Extraits: none.");
    // The English interface keeps the English word.
    expect(localiseInternalNames("Nothing in the excerpts.", english)).toBe("Nothing in the excerpts.");
  });

  it("leaves an ordinary sentence, a number and a citation alone", () => {
    const text = "Le total est de 1 450 € [1]. Votre dossier de travail contient 3 fichiers.";
    expect(localiseInternalNames(text, french)).toBe(text);
    expect(localiseInternalNames("The work folder is empty.", english)).toBe("The work folder is empty.");
  });
});

/**
 * The guard the plan asked for (`docs/test-reports/human-acceptance-pass-1/08-lot-c-plan.md`,
 * C-a1): every upper-case identifier or block label in the strings a model reads must be covered, so
 * a future block name cannot reach the user unnoticed.
 */
describe("every internal name a model reads", () => {
  const HERE = fileURLToPath(new URL(".", import.meta.url));
  const RUST = resolve(HERE, "../../src-tauri/src");
  // Production code only, comments removed: a quotation mark inside a comment would misalign the
  // string literals below.
  const read = (file: string) =>
    (readFileSync(resolve(RUST, file), "utf8").split("#[cfg(test)]")[0] ?? "").replace(/^\s*\/\/.*$/gm, "");

  it("has a display name", () => {
    const covered = (name: string) =>
      COVERED_INTERNAL_NAMES.some((known) => known.toLowerCase().replace(/[_ ]/g, "") === name.toLowerCase().replace(/[_ ]/g, ""));
    // Identifiers written in capitals with underscores inside the strings sent to a model.
    const sources = ["work_folder_context.rs", "mixed_answer.rs", "retrieval.rs", "conversation.rs"];
    for (const file of sources) {
      const strings = [...read(file).matchAll(/"([^"]*)"/g)].map((match) => match[1] ?? "");
      for (const raw of strings) {
        // `{NAME}` in a format string is a Rust placeholder, replaced by the constant's text: never
        // an identifier the model reads.
        const text = raw.replace(/\{\w+\}/g, "");
        for (const [name] of text.matchAll(/\b[A-Z]{2,}(?:_[A-Z]{2,})+\b/g)) {
          expect(covered(name), `${file}: ${name} has no display name in lib/displayNames.ts`).toBe(true);
        }
      }
    }
    // The two block labels of the mixed tier.
    const mixed = read("mixed_answer.rs");
    for (const label of ["Document excerpts", "Table results"]) {
      expect(mixed.includes(label)).toBe(true);
      expect(covered(label), label).toBe(true);
    }
  });
});
