import { describe, expect, it } from "vitest";
import {
  CATALOGUES,
  FALLBACK_LOCALE,
  REFERENCE_LOCALE,
  resolveLocale,
  SUPPORTED_LOCALES,
} from "./catalogues";
import { catalogueKeys, lookupKey, type Catalogue } from "./translate";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const referenceKeys = catalogueKeys(reference).sort();

function text(catalogue: Catalogue, key: string): string {
  const value = lookupKey(catalogue, key);
  if (value === undefined) {
    throw new Error(`missing key: ${key}`);
  }
  return value;
}

function placeholders(template: string): string[] {
  return (template.match(/\{\w+\}/g) ?? []).sort();
}

describe("catalogue parity", () => {
  it.each(SUPPORTED_LOCALES)("%s has exactly the reference keys", (locale) => {
    const keys = catalogueKeys(CATALOGUES[locale] as Catalogue).sort();

    expect(keys).toStrictEqual(referenceKeys);
  });

  it.each(SUPPORTED_LOCALES)("%s uses the same placeholders as the reference", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of referenceKeys) {
      // A translation that drops `{path}` would hide the folder the message is about.
      expect(placeholders(text(catalogue, key)), key).toStrictEqual(
        placeholders(text(reference, key)),
      );
    }
  });

  it.each(SUPPORTED_LOCALES)("%s leaves no string empty", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of referenceKeys) {
      expect(text(catalogue, key).trim(), key).not.toBe("");
    }
  });

  it("does not call the AI a server in the French catalogue", () => {
    const french = CATALOGUES["fr-FR"] as Catalogue;

    for (const key of referenceKeys) {
      expect(text(french, key).toLowerCase(), key).not.toMatch(/\bserveur\b/);
    }
  });

  it("does not call the AI a server in the English catalogue", () => {
    const english = CATALOGUES["en-US"] as Catalogue;

    for (const key of referenceKeys) {
      expect(text(english, key).toLowerCase(), key).not.toMatch(/\bserver\b/);
    }
  });
});

describe("resolveLocale", () => {
  it("prefers what the user chose", () => {
    expect(resolveLocale("en-US", "fr-FR")).toBe("en-US");
  });

  it("falls back to the system locale on a fresh installation", () => {
    expect(resolveLocale(null, "en-US")).toBe("en-US");
  });

  it("accepts a regional variant of a language it ships", () => {
    expect(resolveLocale(null, "fr-BE")).toBe("fr-FR");
    expect(resolveLocale(null, "en-GB")).toBe("en-US");
  });

  it("falls back to the pilot language rather than to the reference one", () => {
    expect(resolveLocale(null, "de-DE")).toBe(FALLBACK_LOCALE);
    expect(FALLBACK_LOCALE).not.toBe(REFERENCE_LOCALE);
  });
});
