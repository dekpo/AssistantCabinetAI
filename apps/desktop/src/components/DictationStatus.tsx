import { useTranslation } from "../i18n/I18nProvider";
import { pluralise } from "../lib/plural";
import type { DictationBridge } from "../speech/useDictation";

/**
 * The line under the question box that says what the dictation is doing, with the one button that
 * fits: Cancel while it works, Undo once the words are in, Close on a problem. It is a polite live
 * region, so a screen reader hears each step; it is empty (and takes no room) when nothing is going on.
 */
export function DictationStatus({ dictation }: { dictation: DictationBridge }) {
  const { t, locale } = useTranslation();
  const { phase, error, result } = dictation;
  const working = phase === "requesting-permission" || phase === "recording" || phase === "transcribing";

  const lines: string[] = [];
  if (phase === "requesting-permission") {
    lines.push(t("speech.dictation.requesting"));
  } else if (phase === "recording") {
    lines.push(t("speech.dictation.recording"));
  } else if (phase === "transcribing") {
    lines.push(t("speech.dictation.transcribing"));
  } else if (phase === "done" && result !== null) {
    lines.push(t("speech.dictation.done"));
    if (result.clipped) {
      lines.push(t("speech.dictation.clipped"));
    }
    if (result.numbers > 0) {
      lines.push(
        t(pluralise(result.numbers, "speech.dictation.numbersOne", "speech.dictation.numbersMany"), {
          count: result.numbers,
        }),
      );
    }
  } else if (phase === "error" && error !== null) {
    lines.push(t(error.key, { locale, ...error.values }));
  }
  if (dictation.analysing && (phase === "recording" || phase === "transcribing")) {
    lines.push(t("speech.dictation.analysing"));
  }

  return (
    <div className="composer__speech">
      <p className="composer__speech-text" role="status" aria-live="polite">
        {lines.join(" ")}
      </p>
      {working ? (
        <button type="button" className="button button--compact" onClick={dictation.cancel}>
          {t("speech.dictation.cancel")}
        </button>
      ) : null}
      {phase === "done" && result !== null ? (
        <button type="button" className="button button--compact" onClick={dictation.undo}>
          {t("speech.dictation.undo")}
        </button>
      ) : null}
      {phase === "error" ? (
        <button type="button" className="button button--compact" onClick={dictation.cancel}>
          {t("speech.dictation.dismiss")}
        </button>
      ) : null}
    </div>
  );
}
