import { useCallback, useEffect, useState } from "react";
import { resolveLocale } from "../i18n/catalogues";
import { normaliseError, type AppError } from "../lib/errors";
import {
  loadAppSnapshot,
  resetSettings,
  saveSettings,
  type AppSettings,
  type AppSnapshot,
} from "../lib/ipc";

export interface AppSettingsState {
  snapshot: AppSnapshot | null;
  /** The locale actually in use: the chosen one, or the system one, or the fallback. */
  locale: string;
  loadError: AppError | null;
  saveError: AppError | null;
  reload: () => void;
  update: (patch: Partial<AppSettings>) => Promise<void>;
  /** Every setting back to a first launch, the documents folder included. Rejects on failure so
   * the caller can keep its confirmation open rather than closing it over an error. */
  reset: () => Promise<void>;
}

export function useAppSettings(): AppSettingsState {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [loadError, setLoadError] = useState<AppError | null>(null);
  const [saveError, setSaveError] = useState<AppError | null>(null);

  const reload = useCallback(() => {
    setLoadError(null);
    loadAppSnapshot()
      .then(setSnapshot)
      .catch((raw: unknown) => setLoadError(normaliseError(raw)));
  }, []);

  useEffect(reload, [reload]);

  const update = useCallback(
    async (patch: Partial<AppSettings>) => {
      if (snapshot === null) {
        return;
      }
      setSaveError(null);
      const merged = { ...snapshot.settings, ...patch };
      try {
        const stored = await saveSettings(merged);
        setSnapshot({ ...snapshot, settings: stored });
      } catch (raw: unknown) {
        setSaveError(normaliseError(raw));
      }
    },
    [snapshot],
  );

  /* Not `update` with a hand-built patch: the defaults belong to Rust, which reads some of them
     from the environment. The stored result comes back and replaces what is on screen, so the
     locale falls back to the system's again and the folder card returns to offering one. */
  const reset = useCallback(async () => {
    if (snapshot === null) {
      return;
    }
    setSaveError(null);
    try {
      const stored = await resetSettings();
      setSnapshot({ ...snapshot, settings: stored });
    } catch (raw: unknown) {
      setSaveError(normaliseError(raw));
      throw raw;
    }
  }, [snapshot]);

  return {
    snapshot,
    locale: resolveLocale(snapshot?.settings.locale, snapshot?.systemLocale),
    loadError,
    saveError,
    reload,
    update,
    reset,
  };
}
