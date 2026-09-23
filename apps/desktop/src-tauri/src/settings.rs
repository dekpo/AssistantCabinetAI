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

/// Embedding weights are not chat weights, so indexing asks for its own alias
/// (`docs/ARCHITECTURE.md`).
const DEFAULT_EMBEDDING_ALIAS: &str = "cabinet-embed";

/// What to assume when no language has been chosen yet. The pilot practice is French
/// (`docs/LANGUAGE-AND-LOCALE.md`); the interface still resolves the system locale first, and
/// this only covers the moment before it has.
pub const DEFAULT_LOCALE: &str = "fr-FR";

/// How long an answer may go **silent** before the client gives up on it, in seconds.
///
/// Silence, not duration: an answer that keeps arriving never expires, however long it takes.
/// The value has to cover the slowest part, which is not the writing but the reading - a large
/// model on the practice's 2019 workstation spends minutes on a long prompt before the first
/// word appears (`docs/HARDWARE.md`).
pub const DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS: u64 = 300;

/// Low enough that a genuinely stuck model is still reported in a reasonable time, high enough
/// that nobody can set a value that cuts off a working answer on a slow machine.
pub const MIN_ANSWER_IDLE_TIMEOUT_SECONDS: u64 = 30;
pub const MAX_ANSWER_IDLE_TIMEOUT_SECONDS: u64 = 3_600;

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
    pub embedding_alias: String,
    pub work_folder: Option<String>,
    /// Seconds of silence before an answer is abandoned. A setting rather than a constant,
    /// because how long a model stays quiet depends on the model and on the machine, and neither
    /// is knowable from here (`.cursor/rules/v0-sprint.mdc`: nothing hardcoded).
    pub answer_idle_timeout_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            locale: None,
            theme: Theme::System,
            server_url: std::env::var(SERVER_URL_ENVIRONMENT_VARIABLE)
                .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string()),
            model_alias: DEFAULT_MODEL_ALIAS.to_string(),
            embedding_alias: DEFAULT_EMBEDDING_ALIAS.to_string(),
            work_folder: None,
            answer_idle_timeout_seconds: DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS,
        }
    }
}

impl Settings {
    /// The stored value as a `Duration`, clamped on the way out as well as on the way in: a
    /// `settings.json` edited by hand never reaches the gateway client unbounded.
    pub fn answer_idle_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.answer_idle_timeout_seconds.clamp(
            MIN_ANSWER_IDLE_TIMEOUT_SECONDS,
            MAX_ANSWER_IDLE_TIMEOUT_SECONDS,
        ))
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
        return (
            Settings::default(),
            vec![AppError::SettingsReadFailed.code().to_string()],
        );
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
pub fn save(
    app: &AppHandle,
    policy: &WorkFolderPolicy,
    settings: Settings,
) -> Result<Settings, AppError> {
    let mut checked = settings;
    checked.server_url = checked.server_url.trim().to_string();
    check_server_url(&checked.server_url)?;
    checked.model_alias = checked.model_alias.trim().to_string();
    checked.embedding_alias = checked.embedding_alias.trim().to_string();
    // Clamped rather than refused: a value typed into the interface, or left behind by a
    // hand-edited file, must not be able to make every answer die early or hang for ever.
    checked.answer_idle_timeout_seconds = checked.answer_idle_timeout_seconds.clamp(
        MIN_ANSWER_IDLE_TIMEOUT_SECONDS,
        MAX_ANSWER_IDLE_TIMEOUT_SECONDS,
    );
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
        && url
            .split("://")
            .nth(1)
            .is_some_and(|rest| !rest.trim().is_empty());
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
            embedding_alias: "cabinet-embed".into(),
            work_folder: Some("D:\\work".into()),
            answer_idle_timeout_seconds: DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS,
        };

        let json = serde_json::to_value(&settings).expect("serialises");

        assert_eq!(json["locale"], "fr-FR");
        assert_eq!(json["theme"], "system");
        assert_eq!(json["serverUrl"], "http://mac-mini.local:8080");
        assert_eq!(json["modelAlias"], "cabinet-chat");
        assert_eq!(json["workFolder"], "D:\\work");
        assert_eq!(
            json["answerIdleTimeoutSeconds"],
            DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn an_idle_timeout_is_clamped_rather_than_trusted() {
        let mut settings = Settings {
            answer_idle_timeout_seconds: 0,
            ..Settings::default()
        };
        assert_eq!(
            settings.answer_idle_timeout(),
            std::time::Duration::from_secs(MIN_ANSWER_IDLE_TIMEOUT_SECONDS)
        );

        settings.answer_idle_timeout_seconds = u64::MAX;
        assert_eq!(
            settings.answer_idle_timeout(),
            std::time::Duration::from_secs(MAX_ANSWER_IDLE_TIMEOUT_SECONDS)
        );

        settings.answer_idle_timeout_seconds = 120;
        assert_eq!(
            settings.answer_idle_timeout(),
            std::time::Duration::from_secs(120)
        );
    }

    #[test]
    fn a_file_written_before_the_setting_existed_still_loads() {
        // `serde(default)` on the struct: an older `settings.json` carries no idle timeout, and
        // must open with the default rather than refuse to load.
        let settings: Settings =
            serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");

        assert_eq!(
            settings.answer_idle_timeout_seconds,
            DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS
        );
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
