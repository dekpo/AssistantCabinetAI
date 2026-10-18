import { useSyncExternalStore } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { DictationBridge } from "../speech/useDictation";
import { MicrophoneGlyph } from "./MicrophoneGlyph";
import { StopGlyph } from "./StopGlyph";

/** The input level of a voice runs low (a speaking voice sits around 0.05 to 0.3), so it is stretched
 * before it sets the size of the ring. */
const LEVEL_GAIN = 4;

/**
 * The microphone of the question box: press to start dictating, press again to finish. While the
 * microphone is opening or the words are being worked out the button waits (the line under the box
 * has the Cancel). It is drawn only when the composer is given a dictation, so with the voice off the
 * box has no microphone at all.
 */
export function MicButton({
  dictation,
  onStart,
}: {
  dictation: DictationBridge;
  /** Reads the caret and the selection of the box and starts: only the box knows them. */
  onStart: () => void;
}) {
  const { t, locale } = useTranslation();
  const level = useSyncExternalStore(dictation.level.subscribe, dictation.level.get, dictation.level.get);
  const recording = dictation.phase === "recording";
  const waiting = dictation.phase === "requesting-permission" || dictation.phase === "transcribing";
  const label = t(recording ? "speech.dictation.stop" : "speech.dictation.start");
  const title = dictation.reasonKey === null ? label : t(dictation.reasonKey, { locale });

  return (
    <button
      type="button"
      className="button button--icon composer__mic"
      aria-label={label}
      title={title}
      aria-pressed={recording}
      aria-busy={waiting}
      disabled={dictation.disabled || waiting}
      style={recording ? { ["--mic-level" as string]: Math.min(1, level * LEVEL_GAIN) } : undefined}
      onClick={recording ? dictation.stop : onStart}
    >
      {recording ? <StopGlyph /> : <MicrophoneGlyph />}
    </button>
  );
}
