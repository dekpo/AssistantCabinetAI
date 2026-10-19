import { useCallback, useEffect, useState } from "react";
import { resolveLocale } from "../i18n/catalogues";
import { normaliseError, type AppError } from "../lib/errors";
import {
  loadAppSnapshot,
  resetSettings,
  saveSettings,
  type AppSettings,
  type AppSnapshot,
  type ExplorerSettings,
  type PanelLayouts,
  type SpeechSettings,
} from "../lib/ipc";
import { sameLayouts } from "../lib/panelLayout";
import { createSettingsQueue } from "../lib/settingsQueue";
import { speechPatch } from "../lib/speechSave";

export interface AppSettingsState {
  snapshot: AppSnapshot | null;
  /** The locale actually in use: the chosen one, or the system one, or the fallback. */
  locale: string;
  loadError: AppError | null;
  saveError: AppError | null;
  reload: () => void;
  /** Stable between renders. Saves are made one after the other, each merged into the settings as
   * they stand when its turn comes (`lib/settingsQueue.ts`). */
  update: (patch: Partial<AppSettings>) => Promise<void>;
  /** The widths of the zones, saved at the end of a gesture. A convenience: a failure is not
   * reported anywhere, and nothing is saved when the layouts did not change. */
  saveLayout: (change: (current: PanelLayouts) => PanelLayouts) => Promise<void>;
  /** One change to the explorer's remembered state (a folded list), applied to it as it stands when
   * its turn comes: `update({ explorer })` would replace the block with a possibly stale copy. A
   * convenience: a failure is not reported anywhere, and nothing is saved when nothing changed. */
  saveExplorer: (change: (current: ExplorerSettings) => ExplorerSettings) => Promise<void>;
  /** One change to the nested `speech` settings, applied to them as they stand when its turn
   * comes: `update({ speech })` would replace the whole block with a possibly stale copy. A
   * failure is reported like `update`'s. */
  saveSpeech: (change: (current: SpeechSettings) => SpeechSettings) => Promise<void>;
  /** Every setting back to a first launch, the documents folder included. Rejects on failure so
   * the caller can keep its confirmation open rather than closing it over an error. */
  reset: () => Promise<void>;
}

export function useAppSettings(): AppSettingsState {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [loadError, setLoadError] = useState<AppError | null>(null);
  const [saveError, setSaveError] = useState<AppError | null>(null);
  /* One queue for the life of the window. What it stores is shown the moment it is stored. */
  const [queue] = useState(() =>
    createSettingsQueue<AppSettings>({
      save: saveSettings,
      reset: resetSettings,
      onStored: (settings) =>
        setSnapshot((current) => (current === null ? current : { ...current, settings })),
    }),
  );

  /* Read in turn with the saves, so a reload cannot slip between two of them. */
  const reload = useCallback(() => {
    setLoadError(null);
    queue
      .enqueue(async () => {
        const loaded = await loadAppSnapshot();
        queue.adopt(loaded.settings);
        setSnapshot(loaded);
      })
      .catch((raw: unknown) => setLoadError(normaliseError(raw)));
  }, [queue]);

  useEffect(reload, [reload]);

  const update = useCallback(
    async (patch: Partial<AppSettings>) => {
      setSaveError(null);
      try {
        await queue.update(patch);
      } catch (raw: unknown) {
        setSaveError(normaliseError(raw));
      }
    },
    [queue],
  );

  const saveLayout = useCallback(
    async (change: (current: PanelLayouts) => PanelLayouts) => {
      try {
        await queue.update((current) => {
          const next = change(current.layout);
          return sameLayouts(next, current.layout) ? null : { layout: next };
        });
      } catch {
        // A layout is a convenience: it is not worth a banner.
      }
    },
    [queue],
  );

  const saveExplorer = useCallback(
    async (change: (current: ExplorerSettings) => ExplorerSettings) => {
      try {
        await queue.update((current) => {
          const next = change(current.explorer);
          return next.density === current.explorer.density &&
            next.documentsFolded === current.explorer.documentsFolded &&
            next.tablesFolded === current.explorer.tablesFolded
            ? null
            : { explorer: next };
        });
      } catch {
        // What is folded is a convenience: it is not worth a banner.
      }
    },
    [queue],
  );

  const saveSpeech = useCallback(
    async (change: (current: SpeechSettings) => SpeechSettings) => {
      setSaveError(null);
      try {
        await queue.update((current) => speechPatch(current, change));
      } catch (raw: unknown) {
        setSaveError(normaliseError(raw));
      }
    },
    [queue],
  );

  /* Not `update` with a hand-built patch: the defaults belong to Rust, which reads some of them
     from the environment. The stored result comes back and replaces what is on screen, so the
     locale falls back to the system's again and the folder card returns to offering one. */
  const reset = useCallback(async () => {
    setSaveError(null);
    try {
      await queue.reset();
    } catch (raw: unknown) {
      setSaveError(normaliseError(raw));
      throw raw;
    }
  }, [queue]);

  return {
    snapshot,
    locale: resolveLocale(snapshot?.settings.locale, snapshot?.systemLocale),
    loadError,
    saveError,
    reload,
    update,
    saveLayout,
    saveExplorer,
    saveSpeech,
    reset,
  };
}
