//! `settings.json` in the application configuration folder.
//!
//! The path comes from the platform, never from a literal: `%APPDATA%\AssistantCabinetAI` on
//! Windows, `~/Library/Application Support/AssistantCabinetAI` on macOS. The server address, the
//! model alias, the locale and the work folder all live here, so none of them is a constant.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::knowledge::packs::{available_pack_ids, canonical_pack_id, pack_domain};
use crate::knowledge::KnowledgeMode;
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

/// The subfolder of the documents folder where generated letters are written, until the user names
/// another (`Settings::generated_folder_name`).
pub const DEFAULT_GENERATED_FOLDER_NAME: &str = "Generated";

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
    /// The Data Folder (CSV/XLSX), a sibling of `work_folder` rather than a rename of it: the two
    /// are validated through the same `WorkFolderPolicy` but kept as separate settings so neither
    /// folder can be lost by the other's absence.
    pub data_folder: Option<String>,
    /// Seconds of silence before an answer is abandoned. A setting rather than a constant,
    /// because how long a model stays quiet depends on the model and on the machine, and neither
    /// is knowable from here (`.cursor/rules/v0-sprint.mdc`: nothing hardcoded).
    pub answer_idle_timeout_seconds: u64,
    /// The subfolder of the documents folder the generated letters are written in. A setting, not a
    /// constant: the practice names it in its own language. Always a single clean name (ASCII letters,
    /// digits, `-`, `_`, `.`), whatever was typed or left in a hand-edited file
    /// (`Settings::generated_folder`).
    pub generated_folder_name: String,
    /// Reveals the developer panel that shows each question's timings. Nothing in the interface
    /// reads it yet; a later lot renders the panel (`docs/DECISIONS.md`, "Knowledge Base"). Never
    /// shown in normal chat.
    pub show_diagnostics: bool,
    /// Appends one line of numbers and machine codes per question to `retrieval-timings.jsonl`,
    /// and one per Analyse pass to `analysis-timings.jsonl`, in the application's local data
    /// folder. No question, no file name, no entity, no excerpt
    /// (`knowledge::diagnostics`). Off by default; edited in `settings.json` until the settings
    /// dialog gets its "Advanced" group.
    pub write_timing_log: bool,
    /// Whether the knowledge base reads the analysed files for names (`knowledge::KnowledgeMode`).
    /// `suggest` by default; `off` is the kill switch that leaves the product as it was before the
    /// knowledge base and writes no `kb_*` row. Edited in `settings.json` until the settings dialog
    /// gets its "Advanced" group.
    pub knowledge_mode: KnowledgeMode,
    /// The optional lexicon packs the knowledge base reads names with, on top of the neutral base.
    /// A pack id is `<domain>-<country>`: `health-fr`, `health-ch`, `legal-fr`, `accounting-fr`,
    /// and later `health-eu`, `legal-ch`... (`docs/DECISIONS.md`, "Knowledge packs are named
    /// domain-country"). The product serves health professionals in France and in Switzerland, so
    /// `health-fr` is on by default and the Swiss module is a choice in the settings. Only the ids a
    /// build ships are kept, and an id written by an earlier build (`health`, `legal`,
    /// `accounting`, which meant the French pack) is read as its `-fr` name
    /// (`Settings::knowledge_pack_ids`).
    pub knowledge_packs: Vec<String>,
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
            data_folder: None,
            answer_idle_timeout_seconds: DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS,
            generated_folder_name: DEFAULT_GENERATED_FOLDER_NAME.to_string(),
            show_diagnostics: false,
            write_timing_log: false,
            knowledge_mode: KnowledgeMode::default(),
            knowledge_packs: vec!["health-fr".to_string()],
        }
    }
}

impl Settings {
    /// The pack ids to load: the stored ones that this build ships, once each, in the stored order.
    /// A `settings.json` edited by hand, or written by a newer build, cannot make the knowledge base
    /// refuse to start over a pack that does not exist.
    pub fn knowledge_pack_ids(&self) -> Vec<String> {
        let shipped = available_pack_ids();
        let mut kept: Vec<String> = Vec::new();
        for stored in &self.knowledge_packs {
            let id = canonical_pack_id(stored);
            // One pack per domain: the modules of two countries are never mixed, so a second
            // `health-*` is dropped whatever its country (the first one stored wins).
            let domain = pack_domain(id);
            if shipped.contains(&id) && !kept.iter().any(|known| pack_domain(known) == domain) {
                kept.push(id.to_string());
            }
        }
        kept
    }

    /// The generated-letters folder name as it is used: one clean name, never a path, never empty.
    /// Sanitised on the way out as well as on the way in, so a `settings.json` edited by hand cannot
    /// make the program write outside the documents folder.
    pub fn generated_folder(&self) -> String {
        clean_folder_name(&self.generated_folder_name)
    }

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
        Ok(Ok(mut settings)) => {
            // An id written by an earlier build is shown, and later saved, under its current name.
            settings.knowledge_packs = settings.knowledge_pack_ids();
            (settings, Vec::new())
        }
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
    checked.generated_folder_name = clean_folder_name(&checked.generated_folder_name);
    checked.knowledge_packs = checked.knowledge_pack_ids();
    checked.work_folder = match checked.work_folder.as_deref() {
        None => None,
        Some(chosen) if chosen.trim().is_empty() => None,
        Some(chosen) => Some(display(&policy.validate(Path::new(chosen))?)),
    };
    checked.data_folder = match checked.data_folder.as_deref() {
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

/// One clean folder name from whatever was typed: the clean-name alphabet, no separator, at most 60
/// characters, and the default when nothing usable is left. A name made only of dots is not a name
/// (`.` and `..` mean "here" and "above").
fn clean_folder_name(typed: &str) -> String {
    use unicode_normalization::char::is_combining_mark;
    use unicode_normalization::UnicodeNormalization;

    let mut cleaned = String::new();
    for ch in typed.trim().nfd() {
        if is_combining_mark(ch) {
            continue;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.') {
            cleaned.push(ch);
        } else if !cleaned.ends_with('-') {
            // Every separator, space and character outside the alphabet becomes one hyphen.
            cleaned.push('-');
        }
    }
    let bounded: String = cleaned.chars().take(60).collect();
    // No leading or trailing dot or hyphen: `.` and `..` mean "here" and "above", and never a name.
    let name = bounded.trim_matches(|ch| ch == '-' || ch == '.');
    if name.chars().any(|ch| ch.is_ascii_alphanumeric()) {
        name.to_string()
    } else {
        DEFAULT_GENERATED_FOLDER_NAME.to_string()
    }
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

    /// What `reset_settings` hands back. The folder is part of it on purpose: a reset that keeps
    /// the folder is not a reset, and the folder is the setting most likely to be part of
    /// whatever went wrong - a disk that is no longer there, or a folder a sync client has since
    /// taken over.
    #[test]
    fn the_defaults_a_reset_restores_keep_no_work_folder_and_no_chosen_language() {
        let defaults = Settings::default();

        assert_eq!(defaults.work_folder, None);
        assert_eq!(defaults.data_folder, None);
        // `None` means "follow the system", which is what a first launch does.
        assert_eq!(defaults.locale, None);
        assert_eq!(defaults.model_alias, DEFAULT_MODEL_ALIAS);
        assert_eq!(defaults.embedding_alias, DEFAULT_EMBEDDING_ALIAS);
        assert_eq!(
            defaults.answer_idle_timeout_seconds,
            DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS
        );
        // The address is a default, not a constant: an installation can set it in the
        // environment, and a reset must restore that rather than a literal typed here.
        assert!(check_server_url(&defaults.server_url).is_ok());
    }

    #[test]
    fn the_stored_shape_is_the_one_the_interface_expects() {
        let settings = Settings {
            locale: Some("fr-FR".into()),
            theme: Theme::System,
            server_url: "http://mac-mini.local:8080".into(),
            model_alias: "cabinet-chat".into(),
            embedding_alias: "cabinet-embed".into(),
            work_folder: Some("D:\\work".into()),
            data_folder: Some("D:\\data".into()),
            answer_idle_timeout_seconds: DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS,
            generated_folder_name: "Courriers".into(),
            show_diagnostics: true,
            write_timing_log: true,
            knowledge_mode: KnowledgeMode::Auto,
            knowledge_packs: vec!["health-ch".into(), "legal-fr".into()],
        };

        let json = serde_json::to_value(&settings).expect("serialises");
        assert_eq!(json["generatedFolderName"], "Courriers");
        assert_eq!(json["showDiagnostics"], true);
        assert_eq!(json["writeTimingLog"], true);
        assert_eq!(json["knowledgeMode"], "auto");
        assert_eq!(json["knowledgePacks"][0], "health-ch");

        assert_eq!(json["locale"], "fr-FR");
        assert_eq!(json["theme"], "system");
        assert_eq!(json["serverUrl"], "http://mac-mini.local:8080");
        assert_eq!(json["modelAlias"], "cabinet-chat");
        assert_eq!(json["workFolder"], "D:\\work");
        assert_eq!(json["dataFolder"], "D:\\data");
        assert_eq!(
            json["answerIdleTimeoutSeconds"],
            DEFAULT_ANSWER_IDLE_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn the_generated_folder_name_is_always_one_clean_name_and_never_a_path() {
        for (typed, expected) in [
            ("Generated", "Generated"),
            ("Courriers g\u{e9}n\u{e9}r\u{e9}s", "Courriers-generes"),
            ("  Lettres  ", "Lettres"),
            ("", "Generated"),
            ("   ", "Generated"),
            ("..", "Generated"),
            ("../../elsewhere", "elsewhere"),
            ("a/b\\c", "a-b-c"),
        ] {
            let settings = Settings {
                generated_folder_name: typed.to_string(),
                ..Settings::default()
            };
            assert_eq!(settings.generated_folder(), expected, "{typed:?}");
        }
        let long = Settings {
            generated_folder_name: "x".repeat(200),
            ..Settings::default()
        };
        assert_eq!(long.generated_folder().chars().count(), 60);
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
    fn a_data_folder_round_trips_through_serialisation_like_the_work_folder() {
        let settings = Settings {
            data_folder: Some("D:\\data\\AssistantCabinetAI\\Data".into()),
            ..Settings::default()
        };

        let json = serde_json::to_string(&settings).expect("serialises");
        let restored: Settings = serde_json::from_str(&json).expect("deserialises");

        assert_eq!(restored.data_folder, settings.data_folder);
    }

    #[test]
    fn a_file_written_before_the_data_folder_setting_existed_still_loads() {
        // Same `serde(default)` contract as the idle timeout below: an older `settings.json`
        // carries no `dataFolder`, and must open with `None` rather than refuse to load.
        let settings: Settings =
            serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");

        assert_eq!(settings.data_folder, None);
    }

    #[test]
    fn measurement_is_off_by_default_and_for_a_file_that_predates_it() {
        let defaults = Settings::default();
        assert!(!defaults.show_diagnostics);
        assert!(!defaults.write_timing_log);

        let settings: Settings =
            serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");
        assert!(!settings.show_diagnostics);
        assert!(!settings.write_timing_log);

        let switched_on: Settings =
            serde_json::from_str(r#"{"writeTimingLog":true}"#).expect("loads");
        assert!(switched_on.write_timing_log);
        assert!(!switched_on.show_diagnostics);
    }

    #[test]
    fn the_knowledge_base_is_on_in_suggest_mode_by_default_and_for_a_file_that_predates_it() {
        assert_eq!(Settings::default().knowledge_mode, KnowledgeMode::Suggest);
        let old: Settings =
            serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");
        assert_eq!(old.knowledge_mode, KnowledgeMode::Suggest);

        for (text, mode) in [
            ("off", KnowledgeMode::Off),
            ("suggest", KnowledgeMode::Suggest),
            ("auto", KnowledgeMode::Auto),
        ] {
            let body = format!(r#"{{"knowledgeMode":"{text}"}}"#);
            let settings: Settings = serde_json::from_str(&body).expect("loads");
            assert_eq!(settings.knowledge_mode, mode);
        }
    }

    #[test]
    fn the_health_pack_is_on_by_default_and_unknown_packs_are_dropped() {
        assert_eq!(Settings::default().knowledge_pack_ids(), ["health-fr"]);
        let old: Settings =
            serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");
        assert_eq!(old.knowledge_pack_ids(), ["health-fr"]);

        let odd: Settings = serde_json::from_str(
            r#"{"knowledgePacks":["health-ch","nonsense","health-fr","health-ch"]}"#,
        )
        .expect("loads");
        // Two countries of one domain are never mixed: the first stored wins.
        assert_eq!(odd.knowledge_pack_ids(), ["health-ch"]);

        // An id written before the packs were named domain-country meant the French pack.
        let legacy: Settings =
            serde_json::from_str(r#"{"knowledgePacks":["health","legal","accounting","health"]}"#)
                .expect("loads");
        assert_eq!(
            legacy.knowledge_pack_ids(),
            ["health-fr", "legal-fr", "accounting-fr"]
        );

        let none: Settings = serde_json::from_str(r#"{"knowledgePacks":[]}"#).expect("loads");
        assert!(none.knowledge_pack_ids().is_empty());
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
