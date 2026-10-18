import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { AppSettings, SpeechSettings } from "../lib/ipc";
import { errorMessage } from "../lib/errors";
import { boundedNumber } from "../lib/knowledgeSettings";
import { microphoneSupport } from "../audio/recorder";
import { useModelJob } from "../state/useModelJob";
import { useSpeechStatus } from "../state/useSpeechStatus";
import { useSystemVoices } from "../state/useSystemVoices";
import {
  featureAvailability,
  INSTALL_AVAILABLE,
  modelStatusKey,
  overrideLanguages,
  voicesOfResolvedLanguage,
} from "../speech/availability";
import type { SpeechFeature } from "../speech/ipc";
import { SPEECH_LIMITS } from "../speech/limits";
import { jobLine, modelButtons } from "../speech/modelJob";
import { NumberSetting } from "./NumberSetting";

const FEATURES: SpeechFeature[] = ["dictation", "readAloud", "notes"];
const ENGINES = ["piper", "system"] as const;

/** Language names come from the system, in the language being shown. */
function languageName(tag: string, locale: string): string {
  return new Intl.DisplayNames([locale], { type: "language" }).of(tag) ?? tag;
}

/**
 * The "Voice" block of the settings dialog: the switches, the state of the speech model, the
 * language, the caps, the read-aloud choices and the notes folder. It starts nothing by itself: the
 * model is downloaded by the Install button, or checked by Verify (the way to use a model copied by
 * hand), and with no model the block says so and nothing else on screen changes. Every change goes
 * through `onSave`, which applies it to the speech settings as they stand when its turn comes.
 */
export function SpeechSettingsBlock({
  settings,
  onSave,
}: {
  settings: AppSettings;
  onSave: (change: (current: SpeechSettings) => SpeechSettings) => void;
}) {
  const { t, locale } = useTranslation();
  const speech = settings.speech;
  /* Bumped when an install or a verify ends, so the state of the model is read again. */
  const [revision, setRevision] = useState(0);
  const { status, failed } = useSpeechStatus(
    JSON.stringify([speech, settings.locale, settings.workFolder, revision]),
  );
  const settled = useCallback(() => setRevision((current) => current + 1), []);
  const modelJob = useModelJob(settled);
  const [notesFolderDraft, setNotesFolderDraft] = useState(speech.notesFolderName);

  /* Committed on blur, like the other text fields: Rust turns what was typed into one clean name
     and the field then shows the result. */
  useEffect(() => {
    setNotesFolderDraft(speech.notesFolderName);
  }, [speech.notesFolderName]);

  const set = (patch: Partial<SpeechSettings>) => onSave((current) => ({ ...current, ...patch }));
  const microphone = microphoneSupport();
  const buttons = modelButtons(
    modelJob.job,
    status?.sttModel ?? "missing",
    INSTALL_AVAILABLE && status !== null,
  );
  const runningLine = jobLine(modelJob.job);
  const languages = overrideLanguages(status);
  const voices = voicesOfResolvedLanguage(status);
  /* The voices of this system, read only when the system engine is the chosen one. */
  const systemVoices = useSystemVoices(speech.ttsEngine === "system", status?.locale ?? locale);
  /* What stops a switched-on feature, one line each; a feature that is simply off says nothing. */
  const problems = FEATURES.flatMap((feature) => {
    const { reasonKey } = featureAvailability(status, feature, microphone);
    return reasonKey === null ? [] : [{ feature, reasonKey }];
  });
  const featureLabels: Record<SpeechFeature, string> = {
    dictation: t("speech.dictationLabel"),
    readAloud: t("speech.readAloudLabel"),
    notes: t("speech.notesLabel"),
  };

  return (
    <details className="field settings-voice">
      <summary className="disclosure">{t("speech.groupLabel")}</summary>

      <div className="field">
        <label className="field__row">
          <input
            type="checkbox"
            checked={speech.enabled}
            onChange={(event) => set({ enabled: event.target.checked })}
          />
          <span className="field__label">{t("speech.enabledLabel")}</span>
        </label>
        <span className="field__description">{t("speech.enabledDescription")}</span>
      </div>

      <div className="field">
        <label className="field__row">
          <input
            type="checkbox"
            checked={speech.dictationEnabled}
            disabled={!speech.enabled}
            onChange={(event) => set({ dictationEnabled: event.target.checked })}
          />
          <span>{t("speech.dictationLabel")}</span>
        </label>
        <label className="field__row">
          <input
            type="checkbox"
            checked={speech.readAloudEnabled}
            disabled={!speech.enabled}
            onChange={(event) => set({ readAloudEnabled: event.target.checked })}
          />
          <span>{t("speech.readAloudLabel")}</span>
        </label>
      </div>

      <div className="field">
        <label className="field__row">
          <input
            type="checkbox"
            checked={speech.notesEnabled}
            onChange={(event) => set({ notesEnabled: event.target.checked })}
          />
          <span className="field__label">{t("speech.notesLabel")}</span>
        </label>
        <span className="field__description">{t("speech.notesDescription")}</span>
      </div>

      {/* The model is reported whatever the switches say, so a missing or damaged one is never
          hidden behind "voice is off". */}
      <div className="field">
        <span className="field__label">{t("speech.modelLabel")}</span>
        {status === null ? (
          <span className="field__description">
            {failed ? t("speech.statusFailed") : t("speech.statusLoading")}
          </span>
        ) : (
          <span className="field__description">{t(modelStatusKey(status.sttModel))}</span>
        )}
        <div className="field__row">
          <button
            type="button"
            className="button button--compact"
            disabled={buttons.installDisabled}
            onClick={modelJob.install}
          >
            {t("speech.install.action")}
          </button>
          <button
            type="button"
            className="button button--compact"
            disabled={buttons.verifyDisabled}
            onClick={modelJob.verify}
          >
            {t("speech.install.verifyAction")}
          </button>
          {buttons.cancelShown ? (
            <button type="button" className="button button--compact" onClick={modelJob.cancel}>
              {t("speech.install.cancel")}
            </button>
          ) : null}
        </div>
        {runningLine === null ? null : (
          <span className="field__description" role="status">
            {t(runningLine.key, runningLine.values)}
          </span>
        )}
        {modelJob.job.kind === "failed" ? (
          <span className="field__description" role="alert">
            {errorMessage(t, modelJob.job.error)}
          </span>
        ) : null}
        {status === null || settings.speech.speechModelsUrl === "" ? null : (
          <span className="field__description">
            {t("speech.install.source", { address: settings.speech.speechModelsUrl })}
          </span>
        )}
      </div>

      {problems.length === 0 ? null : (
        <div className="field">
          {problems.map(({ feature, reasonKey }) => (
            <span className="field__description" key={feature}>
              {t("speech.featureProblem", {
                feature: featureLabels[feature],
                reason: t(reasonKey),
              })}
            </span>
          ))}
        </div>
      )}

      <label className="field">
        <span className="field__label">{t("speech.languageLabel")}</span>
        <span className="field__description">{t("speech.languageDescription")}</span>
        <select
          className="field__control"
          value={speech.languageOverride ?? ""}
          onChange={(event) => set({ languageOverride: event.target.value || null })}
        >
          <option value="">{t("speech.languageFollow")}</option>
          {languages.map((language) => (
            <option key={language.id} value={language.id}>
              {languageName(language.id, locale)}
            </option>
          ))}
        </select>
      </label>

      <NumberSetting
        label={t("speech.dictationCapLabel")}
        description={t("speech.dictationCapDescription")}
        value={String(speech.maxDictationSeconds)}
        min={SPEECH_LIMITS.dictationSeconds.min}
        max={SPEECH_LIMITS.dictationSeconds.max}
        step={10}
        unit={t("speech.secondsUnit")}
        commit={(typed) => {
          const next = boundedNumber(
            typed,
            SPEECH_LIMITS.dictationSeconds,
            speech.maxDictationSeconds,
          );
          if (next !== speech.maxDictationSeconds) {
            set({ maxDictationSeconds: next });
          }
          return String(next);
        }}
      />
      <NumberSetting
        label={t("speech.noteCapLabel")}
        description={t("speech.noteCapDescription")}
        value={String(speech.maxNoteSeconds)}
        min={SPEECH_LIMITS.noteSeconds.min}
        max={SPEECH_LIMITS.noteSeconds.max}
        step={60}
        unit={t("speech.secondsUnit")}
        commit={(typed) => {
          const next = boundedNumber(typed, SPEECH_LIMITS.noteSeconds, speech.maxNoteSeconds);
          if (next !== speech.maxNoteSeconds) {
            set({ maxNoteSeconds: next });
          }
          return String(next);
        }}
      />

      <label className="field">
        <span className="field__label">{t("speech.engineLabel")}</span>
        <select
          className="field__control"
          value={speech.ttsEngine ?? ""}
          onChange={(event) =>
            set({ ttsEngine: ENGINES.find((engine) => engine === event.target.value) ?? null })
          }
        >
          <option value="">{t("speech.engine.auto")}</option>
          {ENGINES.map((engine) => (
            <option key={engine} value={engine}>
              {t(`speech.engine.${engine}`)}
            </option>
          ))}
        </select>
      </label>
      {systemVoices === null ? null : (
        <div className="field">
          <span className="field__description">
            {systemVoices.names.length > 0
              ? t("speech.systemVoices.found", { names: systemVoices.names.join(", ") })
              : t("speech.systemVoices.none")}
          </span>
          {systemVoices.ignored === 0 ? null : (
            <span className="field__description">
              {t(
                systemVoices.ignored === 1
                  ? "speech.systemVoices.ignoredOne"
                  : "speech.systemVoices.ignoredMany",
                { count: systemVoices.ignored },
              )}
            </span>
          )}
        </div>
      )}
      <label className="field">
        <span className="field__label">{t("speech.voiceLabel")}</span>
        <select
          className="field__control"
          value={speech.ttsVoice ?? ""}
          onChange={(event) => set({ ttsVoice: event.target.value || null })}
        >
          <option value="">{t("speech.voice.auto")}</option>
          {voices.map((voice) => (
            <option key={voice.id} value={voice.id}>
              {t("speech.voice.option", { gender: t(`speech.gender.${voice.gender}`), id: voice.id })}
            </option>
          ))}
        </select>
      </label>
      <NumberSetting
        label={t("speech.rateLabel")}
        description={t("speech.rateDescription")}
        value={String(speech.ttsRate)}
        min={SPEECH_LIMITS.ttsRate.min}
        max={SPEECH_LIMITS.ttsRate.max}
        step={0.1}
        commit={(typed) => {
          const next = boundedNumber(typed, SPEECH_LIMITS.ttsRate, speech.ttsRate, 2);
          if (next !== speech.ttsRate) {
            set({ ttsRate: next });
          }
          return String(next);
        }}
      />

      <label className="field">
        <span className="field__label">{t("speech.notesFolderLabel")}</span>
        <span className="field__description">{t("speech.notesFolderDescription")}</span>
        <input
          type="text"
          className="field__control"
          value={notesFolderDraft}
          onChange={(event) => setNotesFolderDraft(event.target.value)}
          onBlur={() => set({ notesFolderName: notesFolderDraft })}
        />
      </label>

      <div className="field">
        <label className="field__row">
          <input
            type="checkbox"
            checked={speech.notesVoiceCommands}
            onChange={(event) => set({ notesVoiceCommands: event.target.checked })}
          />
          <span className="field__label">{t("speech.notesCommandsLabel")}</span>
        </label>
        <span className="field__description">{t("speech.notesCommandsDescription")}</span>
      </div>
    </details>
  );
}
