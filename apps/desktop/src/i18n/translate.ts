export type Catalogue = { readonly [key: string]: string | Catalogue };

export type TranslationValues = Record<string, string | number>;

export type Translator = (key: string, values?: TranslationValues) => string;

/** Read `settings.themeLabel` out of a nested catalogue. */
export function lookupKey(catalogue: Catalogue, key: string): string | undefined {
  let current: string | Catalogue | undefined = catalogue;
  for (const part of key.split(".")) {
    if (typeof current !== "object" || current === null) {
      return undefined;
    }
    current = current[part];
  }
  return typeof current === "string" ? current : undefined;
}

/** Replace `{path}` and friends. An unknown placeholder is left as it is, never crashed on. */
export function interpolate(template: string, values?: TranslationValues): string {
  if (!values) {
    return template;
  }
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = values[name];
    return value === undefined ? match : String(value);
  });
}

/** Every catalogue key, flattened. The parity test compares these lists. */
export function catalogueKeys(catalogue: Catalogue, prefix = ""): string[] {
  return Object.entries(catalogue).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === "string" ? [path] : catalogueKeys(value, path);
  });
}

export function createTranslator(catalogue: Catalogue, reference: Catalogue): Translator {
  return (key, values) => {
    // A key missing from a translation shows the reference wording rather than an empty gap.
    const template = lookupKey(catalogue, key) ?? lookupKey(reference, key) ?? key;
    return interpolate(template, values);
  };
}
