import { useCallback, useEffect, useState } from "react";
import { resolveLocale } from "../i18n/catalogues";
import { normaliseError, type AppError } from "../lib/errors";
import { loadAppSnapshot, saveSettings, type AppSettings, type AppSnapshot } from "../lib/ipc";

export interface AppSettingsState {
  snapshot: AppSnapshot | null;
  /** The locale actually in use: the chosen one, or the system one, or the fallback. */
  locale: string;
  loadError: AppError | null;
  saveError: AppError | null;
  reload: () => void;
  update: (patch: Partial<AppSettings>) => Promise<void>;
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

  return {
    snapshot,
    locale: resolveLocale(snapshot?.settings.locale, snapshot?.systemLocale),
    loadError,
    saveError,
    reload,
    update,
  };
}
