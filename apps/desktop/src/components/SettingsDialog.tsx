import { useState } from "react";
import { SUPPORTED_LOCALES } from "../i18n/catalogues";
import { useTranslation } from "../i18n/I18nProvider";
import type { AppError } from "../lib/errors";
import type { AppSettings, ThemeChoice } from "../lib/ipc";
import { ErrorBanner } from "./ErrorBanner";
import { WorkFolderCard } from "./WorkFolderCard";

const THEME_CHOICES: ThemeChoice[] = ["light", "dark", "system"];

/** Language names come from the system, in the language being shown. */
function languageLabel(tag: string, locale: string): string {
  const names = new Intl.DisplayNames([locale], { type: "language" });
  return names.of(tag) ?? tag;
}

export function SettingsDialog({
  settings,
  settingsPath,
  aliases,
  saveError,
  onUpdate,
  onClose,
}: {
  settings: AppSettings;
  settingsPath: string;
  aliases: string[];
  saveError: AppError | null;
  onUpdate: (patch: Partial<AppSettings>) => void;
  onClose: () => void;
}) {
  const { t, locale } = useTranslation();
  const [serverUrlDraft, setServerUrlDraft] = useState(settings.serverUrl);

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

        <WorkFolderCard
          workFolder={settings.workFolder}
          onChosen={(path) => onUpdate({ workFolder: path })}
        />

        {saveError === null ? null : <ErrorBanner error={saveError} />}
        <p className="dialog__note">{t("settings.storedAt", { path: settingsPath })}</p>
      </div>
    </div>
  );
}
