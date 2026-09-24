import { useEffect, useState } from "react";
import { SUPPORTED_LOCALES } from "../i18n/catalogues";
import { useTranslation } from "../i18n/I18nProvider";
import type { AppError } from "../lib/errors";
import type { AppSettings, ThemeChoice } from "../lib/ipc";
import type { IndexingState } from "../state/useIndexing";
import { ConfirmDialog } from "./ConfirmDialog";
import { ErrorBanner } from "./ErrorBanner";
import { WorkFolderCard } from "./WorkFolderCard";

const THEME_CHOICES: ThemeChoice[] = ["light", "dark", "system"];

/* The same bounds Rust clamps to, so the control offers only what will actually be stored. They
   are duplicated rather than fetched because a number input needs them before anything is saved;
   Rust remains the one that decides (`settings.rs`). */
const MIN_IDLE_TIMEOUT_SECONDS = 30;
const MAX_IDLE_TIMEOUT_SECONDS = 3600;

/** Language names come from the system, in the language being shown. */
function languageLabel(tag: string, locale: string): string {
  const names = new Intl.DisplayNames([locale], { type: "language" });
  return names.of(tag) ?? tag;
}

export function SettingsDialog({
  settings,
  settingsPath,
  suggestedWorkFolder,
  aliases,
  saveError,
  indexing,
  onUpdate,
  onReset,
  onClose,
}: {
  settings: AppSettings;
  settingsPath: string;
  suggestedWorkFolder: string | null;
  aliases: string[];
  saveError: AppError | null;
  /** The same analysis pass the sidebar card starts: one pass, wherever it is started from. */
  indexing: IndexingState;
  onUpdate: (patch: Partial<AppSettings>) => void;
  /** Every setting back to a first launch, the documents folder included. Rejects on failure.
   * Written as a method rather than `() => Promise<void>` so the literal guard in
   * `src/guards/sources.test.ts` does not read `> Promise <` as a text node. */
  onReset(): Promise<void>;
  onClose: () => void;
}) {
  const { t, locale } = useTranslation();
  const [serverUrlDraft, setServerUrlDraft] = useState(settings.serverUrl);
  const [idleTimeoutDraft, setIdleTimeoutDraft] = useState(
    String(settings.answerIdleTimeoutSeconds),
  );
  const [confirmingReset, setConfirmingReset] = useState(false);
  const [resetting, setResetting] = useState(false);

  /* The two text fields are committed on blur, so between keystrokes their drafts are the only
     copy of what she typed. A reset replaces the stored values underneath them; without this the
     old text would sit in the fields looking like a setting that survived, and the next blur
     would save it straight back. Keyed on the stored value, so an ordinary save - which trims -
     also puts the trimmed text back in the field. */
  useEffect(() => {
    setServerUrlDraft(settings.serverUrl);
  }, [settings.serverUrl]);

  useEffect(() => {
    setIdleTimeoutDraft(String(settings.answerIdleTimeoutSeconds));
  }, [settings.answerIdleTimeoutSeconds]);

  const confirmReset = async () => {
    setResetting(true);
    try {
      await onReset();
      setConfirmingReset(false);
    } catch {
      // The banner above already carries it; the confirmation closes rather than trapping her.
      setConfirmingReset(false);
    } finally {
      setResetting(false);
    }
  };

  const themeLabels: Record<ThemeChoice, string> = {
    light: t("settings.themeLight"),
    dark: t("settings.themeDark"),
    system: t("settings.themeSystem"),
  };

  return (
    <div className="dialog" role="dialog" aria-label={t("settings.title")}>
      <header className="dialog__header">
        <h2 className="dialog__title">{t("settings.title")}</h2>
        <button type="button" className="button button--quiet" onClick={onClose}>
          {t("actions.close")}
        </button>
      </header>

      <div className="dialog__body">
        <label className="field">
          <span className="field__label">{t("settings.themeLabel")}</span>
          <select
            className="field__control"
            value={settings.theme}
            onChange={(event) => onUpdate({ theme: event.target.value as ThemeChoice })}
          >
            {THEME_CHOICES.map((choice) => (
              <option key={choice} value={choice}>
                {themeLabels[choice]}
              </option>
            ))}
          </select>
        </label>

        <label className="field">
          <span className="field__label">{t("settings.languageLabel")}</span>
          <span className="field__description">{t("settings.languageDescription")}</span>
          <select
            className="field__control"
            value={locale}
            onChange={(event) => onUpdate({ locale: event.target.value })}
          >
            {SUPPORTED_LOCALES.map((tag) => (
              <option key={tag} value={tag}>
                {languageLabel(tag, locale)}
              </option>
            ))}
          </select>
        </label>

        <div className="field">
          <label className="field__label" htmlFor="server-url">
            {t("settings.serverUrlLabel")}
          </label>
          <span className="field__description">{t("settings.serverUrlDescription")}</span>
          <div className="field__row">
            <input
              id="server-url"
              className="field__control"
              value={serverUrlDraft}
              onChange={(event) => setServerUrlDraft(event.target.value)}
            />
            <button
              type="button"
              className="button"
              disabled={serverUrlDraft === settings.serverUrl}
              onClick={() => onUpdate({ serverUrl: serverUrlDraft.trim() })}
            >
              {t("actions.save")}
            </button>
          </div>
        </div>

        <label className="field">
          <span className="field__label">{t("settings.modelAliasLabel")}</span>
          <span className="field__description">
            {aliases.length === 0
              ? t("settings.modelAliasUnavailable")
              : t("settings.modelAliasDescription")}
          </span>
          {aliases.length === 0 ? (
            <input
              className="field__control"
              value={settings.modelAlias}
              onChange={(event) => onUpdate({ modelAlias: event.target.value })}
            />
          ) : (
            <select
              className="field__control"
              value={settings.modelAlias}
              onChange={(event) => onUpdate({ modelAlias: event.target.value })}
            >
              {aliases.map((alias) => (
                <option key={alias} value={alias}>
                  {alias}
                </option>
              ))}
            </select>
          )}
        </label>

        <label className="field">
          <span className="field__label">{t("settings.idleTimeoutLabel")}</span>
          <span className="field__description">{t("settings.idleTimeoutDescription")}</span>
          <div className="field__row">
            <input
              type="number"
              className="field__control field__control--number"
              min={MIN_IDLE_TIMEOUT_SECONDS}
              max={MAX_IDLE_TIMEOUT_SECONDS}
              step={30}
              value={idleTimeoutDraft}
              onChange={(event) => setIdleTimeoutDraft(event.target.value)}
              /* Committed on blur rather than on every keystroke: typing "300" would otherwise
                 save 3, then 30, and Rust would clamp the first two up to the floor. */
              onBlur={() => onUpdate({ answerIdleTimeoutSeconds: Number(idleTimeoutDraft) })}
            />
            <span className="field__unit">{t("settings.idleTimeoutUnit")}</span>
          </div>
        </label>

        <WorkFolderCard
          workFolder={settings.workFolder}
          suggestedWorkFolder={suggestedWorkFolder}
          onChosen={(path) => onUpdate({ workFolder: path })}
          indexing={indexing}
        />

        {saveError === null ? null : <ErrorBanner error={saveError} />}

        {/* Last in the panel, under everything it undoes, with a line saying what it costs
            before she reaches the button rather than only once she has pressed it. */}
        <div className="field">
          <span className="field__label">{t("settings.resetLabel")}</span>
          <span className="field__description">{t("settings.resetDescription")}</span>
          <div className="field__row">
            <button
              type="button"
              className="button button--compact"
              disabled={resetting}
              onClick={() => setConfirmingReset(true)}
            >
              {t("settings.reset")}
            </button>
          </div>
        </div>

        <p className="dialog__note">{t("settings.storedAt", { path: settingsPath })}</p>
      </div>

      {confirmingReset ? (
        <ConfirmDialog
          title={t("settings.resetTitle")}
          body={t("settings.resetBody")}
          confirmLabel={t("settings.reset")}
          destructive
          busy={resetting}
          onConfirm={() => void confirmReset()}
          onCancel={() => setConfirmingReset(false)}
        />
      ) : null}
    </div>
  );
}
