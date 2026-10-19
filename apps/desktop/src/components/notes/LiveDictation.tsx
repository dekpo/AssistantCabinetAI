import { useEffect, useState, useSyncExternalStore } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { pluralise } from "../../lib/plural";
import type { DictationNote } from "../../speech/dictation";
import type { LongPhase } from "../../speech/longDictation";
import type { RepairBridge } from "../../speech/useRepairSuggestions";
import { DictationPill } from "./DictationPill";

/**
 * What the Notes section is given when the voice can write a note: the pill, the line under the
 * note that says what is going on, and the name suggestions. `App` hands `null` while the voice is
 * off, and then none of it is drawn.
 */
export interface NotesDictation {
  phase: LongPhase;
  /** The microphone is open (or being asked for). */
  recording: boolean;
  disabled: boolean;
  /** The catalogue key of the sentence that says why the pill cannot be used, or null. */
  reasonKey: string | null;
  /** The input level on its own channel, so the waveform moves without redrawing the window. */
  level: { get: () => number; subscribe: (listener: () => void) => () => void };
  /** When the recording started (a `Date.now()`), while it runs. */
  startedAt: number | null;
  waiting: number;
  failed: number[];
  error: DictationNote | null;
  capReached: boolean;
  onToggle: () => void;
  /** Ends the recording and drops what was not yet written. */
  onCancel: () => void;
  onDismiss: () => void;
  /** The name suggestions for the dictated words, when the voice is on. */
  repair?: RepairBridge;
}

/** The pill with its live level and its clock: only this component redraws while the level moves. */
export function LivePill({ dictation }: { dictation: NotesDictation }) {
  const level = useSyncExternalStore(dictation.level.subscribe, dictation.level.get, dictation.level.get);
  const [now, setNow] = useState(() => Date.now());
  const running = dictation.startedAt !== null;

  useEffect(() => {
    if (!running) {
      return undefined;
    }
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(id);
  }, [running]);

  return (
    <DictationPill
      recording={dictation.recording}
      level={level}
      elapsedMs={dictation.startedAt === null ? 0 : Math.max(0, now - dictation.startedAt)}
      disabled={dictation.disabled}
      onToggle={dictation.onToggle}
    />
  );
}

/** The passages still to be written, the way out, and what the last recording left to say. */
export function DictationStatus({ dictation }: { dictation: NotesDictation }) {
  const { t } = useTranslation();
  const active = dictation.phase !== "idle";
  const notice = !active && (dictation.error !== null || dictation.failed.length > 0 || dictation.capReached);

  return (
    <div className="notes__dictation">
      {dictation.reasonKey !== null && !active ? (
        <p className="notes__hint">{t(dictation.reasonKey)}</p>
      ) : null}
      {active ? (
        <p className="notes__status" role="status">
          {dictation.waiting > 0 ? (
            <>
              <span className="spinner" aria-hidden="true" />
              {pluralise(
                dictation.waiting,
                t("notes.dictation.waitingOne"),
                t("notes.dictation.waitingMany", { count: dictation.waiting }),
              )}
            </>
          ) : null}
          <button type="button" className="button button--compact button--quiet" onClick={dictation.onCancel}>
            {t("notes.dictation.discard")}
          </button>
        </p>
      ) : null}
      {notice ? (
        <div className="notes__report" role="status">
          {dictation.capReached ? <p className="notes__hint">{t("notes.dictation.capReached")}</p> : null}
          {dictation.failed.length > 0 ? (
            <p className="notes__hint">
              {t(
                pluralise(
                  dictation.failed.length,
                  "notes.dictation.failedOne",
                  "notes.dictation.failedMany",
                ),
                { positions: dictation.failed.join(", ") },
              )}
            </p>
          ) : null}
          {dictation.error === null ? null : (
            <p className="notes__hint">{t(dictation.error.key, dictation.error.values)}</p>
          )}
          <button type="button" className="button button--compact" onClick={dictation.onDismiss}>
            {t("actions.close")}
          </button>
        </div>
      ) : null}
    </div>
  );
}
