//! `settings.json` in the application configuration folder.
//!
//! The path comes from the platform, never from a literal: `%APPDATA%\AssistantCabinetAI` on
//! Windows, `~/Library/Application Support/AssistantCabinetAI` on macOS. The server address, the
//! model alias, the locale and the work folder all live here, so none of them is a constant.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::work_folder::{display, WorkFolderPolicy};

const SETTINGS_FILE_NAME: &str = "settings.json";

/// Where the gateway usually answers on the same machine. It is only the starting value: the
/// practice server lives elsewhere and the user changes it in the settings panel.
const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8080";

/// Read once at first launch, so a development machine can point elsewhere without editing code.
const SERVER_URL_ENVIRONMENT_VARIABLE: &str = "ASSISTANT_CABINET_SERVER_URL";

/// An alias, resolved by the gateway. Never a model weight name.
const DEFAULT_MODEL_ALIAS: &str = "cabinet-chat";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `None` until a language has been chosen. The interface resolves it from the system locale
    /// and stores the result; the gateway falls back to its own default meanwhile.
    pub locale: Option<String>,
    pub theme: Theme,
    pub server_url: String,
    pub model_alias: String,
    pub work_folder: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            locale: None,
            theme: Theme::System,
            server_url: std::env::var(SERVER_URL_ENVIRONMENT_VARIABLE)
                .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string()),
            model_alias: DEFAULT_MODEL_ALIAS.to_string(),
            work_folder: None,
        }
    }
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|_| AppError::SettingsReadFailed)?;
    Ok(directory.join(SETTINGS_FILE_NAME))
}

/// Read the stored settings. An unreadable file is reported as a warning rather than as a
/// failure: the window must open even when the file was hand-edited into nonsense.
pub fn load(app: &AppHandle) -> (Settings, Vec<String>) {
    let Ok(path) = settings_path(app) else {
        return (Settings::default(), vec![AppError::SettingsReadFailed.code().to_string()]);
    };
    if !path.exists() {
        return (Settings::default(), Vec::new());
    }
    match std::fs::read_to_string(&path).map(|text| serde_json::from_str::<Settings>(&text)) {
        Ok(Ok(settings)) => (settings, Vec::new()),
        _ => (
            Settings::default(),
            vec![AppError::SettingsReadFailed.code().to_string()],
        ),
    }
}

/// Validate, then store. The webview is not trusted with the work folder or the address: both are
/// checked here, so a change in the interface cannot widen the allow-list.
pub fn save(app: &AppHandle, policy: &WorkFolderPolicy, settings: Settings) -> Result<Settings, AppError> {
    let mut checked = settings;
    checked.server_url = checked.server_url.trim().to_string();
    check_server_url(&checked.server_url)?;
    checked.model_alias = checked.model_alias.trim().to_string();
    checked.work_folder = match checked.work_folder.as_deref() {
        None => None,
        Some(chosen) if chosen.trim().is_empty() => None,
        Some(chosen) => Some(display(&policy.validate(Path::new(chosen))?)),
    };

    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AppError::SettingsWriteFailed)?;
    }
    let text = serde_json::to_string_pretty(&checked).map_err(|_| AppError::SettingsWriteFailed)?;
    std::fs::write(&path, text).map_err(|_| AppError::SettingsWriteFailed)?;
    Ok(checked)
}

/// A plain shape check. `http` on the practice network, `https` once there are certificates; a
/// remote model service would be neither reachable nor allowed.
pub fn check_server_url(url: &str) -> Result<(), AppError> {
    let looks_usable = (url.starts_with("http://") || url.starts_with("https://"))
        && url.split("://").nth(1).is_some_and(|rest| !rest.trim().is_empty());
    if looks_usable {
        Ok(())
    } else {
        Err(AppError::ServerUrlInvalid {
            url: url.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stored_shape_is_the_one_the_interface_expects() {
        let settings = Settings {
            locale: Some("fr-FR".into()),
            theme: Theme::System,
            server_url: "http://mac-mini.local:8080".into(),
            model_alias: "cabinet-chat".into(),
            work_folder: Some("D:\\work".into()),
        };

        let json = serde_json::to_value(&settings).expect("serialises");

        assert_eq!(json["locale"], "fr-FR");
        assert_eq!(json["theme"], "system");
        assert_eq!(json["serverUrl"], "http://mac-mini.local:8080");
        assert_eq!(json["modelAlias"], "cabinet-chat");
        assert_eq!(json["workFolder"], "D:\\work");
    }

    #[test]
    fn a_partial_file_keeps_the_defaults_for_what_it_does_not_say() {
        let settings: Settings =
            serde_json::from_str(r#"{"locale":"en-US"}"#).expect("deserialises");

        assert_eq!(settings.locale.as_deref(), Some("en-US"));
        assert_eq!(settings.theme, Theme::System);
        assert_eq!(settings.model_alias, "cabinet-chat");
    }

    #[test]
    fn an_address_that_is_not_a_server_is_refused() {
        assert!(check_server_url("http://mac-mini.local:8080").is_ok());
        assert!(check_server_url("https://ia.cabinet.local/v1").is_ok());
        assert_eq!(
            check_server_url("mac-mini.local")
                .expect_err("refused")
                .code(),
            "server_url_invalid"
        );
        assert_eq!(
            check_server_url("http://").expect_err("refused").code(),
            "server_url_invalid"
        );
    }
}
