import enUS from "../locales/en-US.json";
import frFR from "../locales/fr-FR.json";
import type { Catalogue } from "./translate";

/**
 * Adding a language means adding a JSON file and one line here. Nothing else in the client
 * knows which languages exist, and the Rust side never decides: it reports the system locale
 * and this registry says whether a catalogue ships for it.
 */
export const CATALOGUES: Record<string, Catalogue> = {
  "en-US": enUS as Catalogue,
  "fr-FR": frFR as Catalogue,
};

/** The catalogue developers and agents review. Missing keys fall back to it. */
export const REFERENCE_LOCALE = "en-US";

/** Used when the system locale has no catalogue. The pilot practice is French. */
export const FALLBACK_LOCALE = "fr-FR";

export const SUPPORTED_LOCALES = Object.keys(CATALOGUES);

export function isSupportedLocale(tag: string): boolean {
  return tag in CATALOGUES;
}

/**
 * Pick the locale to show, in order: what the user chose, the system locale, the fallback.
 * A tag whose language matches a shipped catalogue is good enough (`fr-BE` shows French).
 */
export function resolveLocale(chosen?: string | null, systemLocale?: string | null): string {
  for (const candidate of [chosen, systemLocale]) {
    if (!candidate) {
      continue;
    }
    if (isSupportedLocale(candidate)) {
      return candidate;
    }
    const language = candidate.split("-")[0]?.toLowerCase();
    const sameLanguage = SUPPORTED_LOCALES.find(
      (supported) => supported.split("-")[0]?.toLowerCase() === language,
    );
    if (sameLanguage) {
      return sameLanguage;
    }
  }
  return FALLBACK_LOCALE;
}
