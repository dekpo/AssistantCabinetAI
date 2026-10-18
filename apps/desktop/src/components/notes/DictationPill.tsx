import { useTranslation } from "../../i18n/I18nProvider";
import { barHeights, formatClock } from "../../lib/waveform";
import { MicrophoneGlyph } from "../MicrophoneGlyph";

/** What the pill needs: it draws, it does not record. The recording is wired in by the dictation phase. */
export interface DictationPillProps {
  recording: boolean;
  /** The input level, 0 to 1, while recording. */
  level: number;
  elapsedMs: number;
  disabled?: boolean;
  onToggle: () => void;
}

/**
 * The dark pill centred at the bottom of the note: a microphone at rest, a waveform driven by the
 * input level and the elapsed time while recording, and a hint that a second click finishes. A
 * single button, so the keyboard reaches it like any other.
 */
export function DictationPill({
  recording,
  level,
  elapsedMs,
  disabled = false,
  onToggle,
}: DictationPillProps) {
  const { t } = useTranslation();

  return (
    <div className="pill">
      <button
        type="button"
        className={recording ? "pill__button pill__button--recording" : "pill__button"}
        aria-pressed={recording}
        aria-label={recording ? t("notes.pill.finish") : t("notes.pill.start")}
        title={recording ? t("notes.pill.finish") : t("notes.pill.start")}
        disabled={disabled}
        onClick={onToggle}
      >
        {recording ? (
          <>
            <span className="pill__bars" aria-hidden="true">
              {barHeights(level).map((height, index) => (
                <span key={index} className="pill__bar" style={{ height: `${height}%` }} />
              ))}
            </span>
            <span className="pill__time">{formatClock(elapsedMs)}</span>
          </>
        ) : (
          <MicrophoneGlyph />
        )}
      </button>
      {recording ? <p className="pill__hint">{t("notes.pill.hint")}</p> : null}
    </div>
  );
}
