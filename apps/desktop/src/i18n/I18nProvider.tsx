import { createContext, useContext, useMemo, type ReactNode } from "react";
import { CATALOGUES, REFERENCE_LOCALE } from "./catalogues";
import { createTranslator, type Catalogue, type Translator } from "./translate";

interface I18nValue {
  locale: string;
  t: Translator;
}

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ locale, children }: { locale: string; children: ReactNode }) {
  const value = useMemo<I18nValue>(() => {
    const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
    const catalogue = CATALOGUES[locale] ?? reference;
    return { locale, t: createTranslator(catalogue, reference) };
  }, [locale]);

  // Screen readers and text selection follow the chosen language.
  document.documentElement.lang = locale;

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useTranslation(): I18nValue {
  const value = useContext(I18nContext);
  if (value === null) {
    throw new Error("useTranslation must be used inside I18nProvider");
  }
  return value;
}
