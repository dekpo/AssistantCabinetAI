import { describe, expect, it } from "vitest";
import { createTranslator, type Catalogue } from "../i18n/translate";
import enUS from "../locales/en-US.json";
import frFR from "../locales/fr-FR.json";
import { unreadFormatsLines } from "./unreadFormats";

const reference = enUS as Catalogue;
const french = createTranslator(frFR as Catalogue, reference);
const english = createTranslator(reference, reference);

describe("the line of the analysis summary about files that were not read", () => {
  it("says nothing when every file was read", () => {
    expect(unreadFormatsLines(french, [])).toEqual([]);
    expect(unreadFormatsLines(french, undefined)).toEqual([]);
  });

  it("counts the files and names their formats, in French", () => {
    expect(
      unreadFormatsLines(french, [
        { extension: "pages", count: 7 },
        { extension: "wps", count: 7 },
        { extension: "msg", count: 2 },
      ]),
    ).toEqual([
      "16 fichiers n'ont pas été lus : leur format n'est pas pris en charge (7 .pages, 7 .wps, 2 .msg).",
    ]);
  });

  it("uses the singular for one file", () => {
    expect(unreadFormatsLines(french, [{ extension: "key", count: 1 }])).toEqual([
      "1 fichier n'a pas été lu : son format n'est pas pris en charge (1 .key).",
    ]);
  });

  it("has an English twin", () => {
    expect(unreadFormatsLines(english, [{ extension: "pages", count: 3 }])).toEqual([
      "3 files were not read: their format is not supported (3 .pages).",
    ]);
  });
});
