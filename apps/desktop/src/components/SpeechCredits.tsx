import { useTranslation } from "../i18n/I18nProvider";
import { COMPONENT_CREDITS, voiceCredits } from "../speech/credits";
import type { SpeechStatus } from "../speech/ipc";

/**
 * The credits and licences of the voice, a block of the voice settings: the components with the
 * notices their licences ask for, then each voice with its licence and its rating. It reads the
 * status Rust already gave the settings and does nothing else.
 */
export function SpeechCredits({ status }: { status: SpeechStatus | null }) {
  const { t } = useTranslation();
  const voices = voiceCredits(status);

  return (
    <details className="field settings-credits">
      <summary className="disclosure">{t("speech.credits.title")}</summary>
      <span className="field__description">{t("speech.credits.intro")}</span>

      <span className="field__label">{t("speech.credits.componentsLabel")}</span>
      {COMPONENT_CREDITS.map((credit) => (
        <span className="field__description" key={credit.id}>
          {t(credit.textKey)}
        </span>
      ))}

      {voices.length === 0 ? null : (
        <>
          <span className="field__label">{t("speech.credits.voicesLabel")}</span>
          {voices.map((voice) => (
            <span className="field__description" key={voice.id}>
              {t(voice.textKey)}
              {" "}
              {t(`speech.rating.${voice.rating}`)}
            </span>
          ))}
        </>
      )}
    </details>
  );
}
