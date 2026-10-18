//! `settings.json` in the application configuration folder.
//!
//! The path comes from the platform, never from a literal: `%APPDATA%\AssistantCabinetAI` on
//! Windows, `~/Library/Application Support/AssistantCabinetAI` on macOS. The server address, the
//! model alias, the locale and the work folder all live here, so none of them is a constant.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::knowledge::packs::{available_pack_ids, canonical_pack_id};
use crate::knowledge::reduce::TargetingPolicy;
use crate::knowledge::KnowledgeMode;
use crate::speech::settings::SpeechSettings;
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

/// Defaults of the targeting settings (`docs/SESSION-KB-00-master.md`, section 8). Named constants,
/// never policy hidden in the code that applies them: the values are settings.
pub const DEFAULT_KNOWLEDGE_MIN_SCOPE_FILES: usize = 8;
pub const DEFAULT_KNOWLEDGE_MAX_SELECTIVITY: f32 = 0.6;
pub const DEFAULT_KNOWLEDGE_MIN_REDUCTION: f32 = 0.3;
pub const DEFAULT_KNOWLEDGE_MAX_CANDIDATES: usize = 15;
/// Measured in lot 6 on the embedding model the pilot runs (`nomic-embed-text`): questions about
/// nothing in the passages reach 0.555 to 0.645 against the closest passage, questions whose
/// passage is there reach 0.698 to 0.843, so 0.65 separates them (`lot-06-widen-calibration.py`,
/// `docs/test-reports/knowledge-base-pass-1/lots/lot-06-targeted-scope.md`, "Measured"). The scale
/// belongs to the embedding model: another model needs the script run again. Lot 11 checks it on
/// the owner's own documents.
pub const DEFAULT_KNOWLEDGE_WIDEN_BELOW_SCORE: f32 = 0.65;

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

/// The widths of the three zones (resources, workspace, chat) as percentages of the window.
type PanelTriple = [f64; 3];

/// The remembered widths of the three zones, one triple per state of the centre zone, so closing and
/// reopening the centre keeps the proportions of each state (`docs/CLIENT.md`, "Layout").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelLayouts {
    /// Saved while the centre zone is open: its share is greater than zero.
    #[serde(deserialize_with = "lenient_triple")]
    pub open: Option<PanelTriple>,
    /// Saved while the centre zone is collapsed: its share is exactly zero.
    #[serde(deserialize_with = "lenient_triple")]
    pub collapsed: Option<PanelTriple>,
}

/// Anything that is not three numbers reads as "nothing saved", so a hand-edited `layout` can never
/// make the whole file unreadable. The values themselves are checked by `PanelLayouts::sanitised`.
fn lenient_triple<'de, D>(deserializer: D) -> Result<Option<PanelTriple>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let Some(items) = value.as_array() else {
        return Ok(None);
    };
    let numbers: Vec<f64> = items.iter().filter_map(serde_json::Value::as_f64).collect();
    Ok(match numbers.as_slice() {
        [resources, workspace, chat] if items.len() == 3 => Some([*resources, *workspace, *chat]),
        _ => None,
    })
}

impl PanelLayouts {
    /// Every unusable triple replaced by `None`: finite values from 0 to 100 adding up to 100 give
    /// or take 1; an `open` triple has a centre greater than zero, a `collapsed` one exactly zero.
    /// The pixel minimums of the zones belong to the interface, not to what is stored.
    pub fn sanitised(&self) -> Self {
        let usable = |triple: &PanelTriple| {
            triple
                .iter()
                .all(|share| share.is_finite() && (0.0..=100.0).contains(share))
                && (triple.iter().sum::<f64>() - 100.0).abs() <= 1.0
        };
        Self {
            open: self.open.filter(|triple| usable(triple) && triple[1] > 0.0),
            collapsed: self
                .collapsed
                .filter(|triple| usable(triple) && triple[1] == 0.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// `health-fr` is on by default and the Swiss module is a choice in the settings; both can be on
    /// (cross-border workers). Only the ids a
    /// build ships are kept, and an id written by an earlier build (`health`, `legal`,
    /// `accounting`, which meant the French pack) is read as its `-fr` name
    /// (`Settings::knowledge_pack_ids`).
    pub knowledge_packs: Vec<String>,
    /// Below this many selected files on one side (documents, or workbooks) the knowledge base does
    /// not reduce that side by itself: with a handful of files there is nothing to save and a lot to
    /// lose. A reduction the user asked for with a chip applies at any size
    /// (`knowledge::reduce`).
    pub knowledge_min_scope_files: usize,
    /// An entity present in more than this share of the selected files names no file in particular
    /// (the user's own name, the letterhead): it is ignored for reduction.
    pub knowledge_max_selectivity: f32,
    /// A reduction applies only when it leaves out at least this share of the selected files.
    pub knowledge_min_reduction: f32,
    /// At most this many documents are searched after a reduction (workbooks are not capped).
    pub knowledge_max_candidates: usize,
    /// When the closest passage of a reduced search is below this cosine similarity (by meaning
    /// alone, without the lexical bonus), the search is repeated once on the whole selection. The
    /// scale belongs to the embedding model: 0.65 is measured on `nomic-embed-text`.
    pub knowledge_widen_below_score: f32,
    /// Question suggestions leave names out: a screen is visible to whoever sits across from the
    /// user (master decision D10). On by default. Read by the suggestions of a later lot; the
    /// setting exists now so the dialog and the stored file already carry it.
    pub knowledge_hide_names_in_suggestions: bool,
    /// The copy buttons of the conversation (one answer, or the whole conversation) also copy the
    /// lists of sources shown under each answer. Read by the client only. Off by default.
    pub copy_sources: bool,
    /// The user's own names (hers and the practice's): a word processor stamps the licence holder on every
    /// document, so a property that holds one of these says nothing about who wrote the file. Edited in
    /// `settings.json` until the settings dialog gets its "Advanced" group; kept clean by `save`.
    pub own_names: Vec<String>,
    /// The remembered widths of the three zones of the window. A convenience of the interface: kept
    /// valid by `PanelLayouts::sanitised` on the way in and on the way out.
    pub layout: PanelLayouts,
    /// Local voice: dictation, read-aloud and voice notes. One nested field, kept valid by
    /// `SpeechSettings::sanitised` on the way in and on the way out (`docs/SPEECH.md`, section 4).
    pub speech: SpeechSettings,
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
            knowledge_min_scope_files: DEFAULT_KNOWLEDGE_MIN_SCOPE_FILES,
            knowledge_max_selectivity: DEFAULT_KNOWLEDGE_MAX_SELECTIVITY,
            knowledge_min_reduction: DEFAULT_KNOWLEDGE_MIN_REDUCTION,
            knowledge_max_candidates: DEFAULT_KNOWLEDGE_MAX_CANDIDATES,
            knowledge_widen_below_score: DEFAULT_KNOWLEDGE_WIDEN_BELOW_SCORE,
            knowledge_hide_names_in_suggestions: true,
            copy_sources: false,
            own_names: Vec::new(),
            layout: PanelLayouts::default(),
            speech: SpeechSettings::default(),
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
            // Any number of countries can be active together (a cross-border worker has a French
            // and a Swiss number): the packs are cumulative.
            if shipped.contains(&id) && !kept.iter().any(|known| known == id) {
                kept.push(id.to_string());
            }
        }
        kept
    }

    /// The targeting settings as they are used: each one clamped, so a `settings.json` edited by hand
    /// can neither make a reduction impossible to stop nor make it swallow a whole selection.
    pub fn targeting_policy(&self) -> TargetingPolicy {
        let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
        TargetingPolicy {
            min_scope_files: self.knowledge_min_scope_files.clamp(1, 10_000),
            max_selectivity: finite(
                self.knowledge_max_selectivity,
                DEFAULT_KNOWLEDGE_MAX_SELECTIVITY,
            )
            .clamp(0.05, 1.0),
            min_reduction: finite(
                self.knowledge_min_reduction,
                DEFAULT_KNOWLEDGE_MIN_REDUCTION,
            )
            .clamp(0.0, 0.95),
            max_candidates: self.knowledge_max_candidates.clamp(1, 500),
            widen_below_score: finite(
                self.knowledge_widen_below_score,
                DEFAULT_KNOWLEDGE_WIDEN_BELOW_SCORE,
            )
            .clamp(0.0, 1.5),
        }
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
            // A hand-edited layout never reaches the webview with a value it cannot use.
            settings.layout = settings.layout.sanitised();
            settings.speech = settings.speech.sanitised();
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
    checked.own_names = clean_own_names(&checked.own_names);
    checked.layout = checked.layout.sanitised();
    checked.speech = checked.speech.sanitised();
    let targeting = checked.targeting_policy();
    checked.knowledge_min_scope_files = targeting.min_scope_files;
    checked.knowledge_max_selectivity = targeting.max_selectivity;
    checked.knowledge_min_reduction = targeting.min_reduction;
    checked.knowledge_max_candidates = targeting.max_candidates;
    checked.knowledge_widen_below_score = targeting.widen_below_score;
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
/// The user's own names as they are kept: trimmed, none empty, none twice, each of a sensible length, and not
/// more than a handful (hers, the practice's, a partner's).
fn clean_own_names(typed: &[String]) -> Vec<String> {
    const MAX_NAMES: usize = 20;
    const MAX_CHARS: usize = 100;
    let mut kept: Vec<String> = Vec::new();
    for name in typed {
        let name: String = name.trim().chars().take(MAX_CHARS).collect();
        if !name.is_empty() && !kept.contains(&name) && kept.len() < MAX_NAMES {
            kept.push(name);
        }
    }
    kept
}

fn clean_folder_name(typed: &str) -> String {
    clean_folder_name_or(typed, DEFAULT_GENERATED_FOLDER_NAME)
}

/// The same cleaning with the name to fall back on when nothing usable is left, for a folder that
/// has its own default (the notes folder).
pub(crate) fn clean_folder_name_or(typed: &str, fallback: &str) -> String {
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
        fallback.to_string()
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
            knowledge_packs: vec!["health-fr".into(), "health-ch".into()],
            knowledge_min_scope_files: 12,
            knowledge_max_selectivity: 0.5,
            knowledge_min_reduction: 0.25,
            knowledge_max_candidates: 20,
            knowledge_widen_below_score: 0.35,
            knowledge_hide_names_in_suggestions: false,
            copy_sources: true,
            own_names: vec!["Zorglub Metadatus".into()],
            layout: PanelLayouts {
                open: Some([20.0, 40.0, 40.0]),
                collapsed: Some([25.0, 0.0, 75.0]),
            },
            speech: SpeechSettings {
                enabled: true,
                notes_folder_name: "Dictees".into(),
                ..SpeechSettings::default()
            },
        };

        let json = serde_json::to_value(&settings).expect("serialises");
        assert_eq!(json["speech"]["enabled"], true);
        assert_eq!(json["speech"]["notesFolderName"], "Dictees");
        assert_eq!(json["layout"]["open"][1], 40.0);
        assert_eq!(json["layout"]["collapsed"][2], 75.0);
        assert_eq!(json["generatedFolderName"], "Courriers");
        assert_eq!(json["showDiagnostics"], true);
        assert_eq!(json["writeTimingLog"], true);
        assert_eq!(json["knowledgeMode"], "auto");
        assert_eq!(json["knowledgePacks"][1], "health-ch");
        assert_eq!(json["knowledgeMinScopeFiles"], 12);
        assert_eq!(json["knowledgeMaxCandidates"], 20);
        assert_eq!(json["knowledgeHideNamesInSuggestions"], false);
        assert_eq!(json["copySources"], true);
        assert_eq!(json["ownNames"][0], "Zorglub Metadatus");
        assert!((json["knowledgeMaxSelectivity"].as_f64().unwrap() - 0.5).abs() < 1e-6);

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
    fn a_layout_that_is_usable_is_kept_as_it_is() {
        let layout = PanelLayouts {
            open: Some([20.0, 40.0, 40.0]),
            collapsed: Some([25.0, 0.0, 75.0]),
        };

        assert_eq!(layout.sanitised(), layout);
        // Adding up to 100 give or take 1 is accepted: the interface rounds what it hands over.
        let rounded = PanelLayouts {
            open: Some([20.4, 39.8, 40.3]),
            collapsed: None,
        };
        assert_eq!(rounded.sanitised(), rounded);
    }

    #[test]
    fn a_layout_that_cannot_be_used_is_dropped_triple_by_triple() {
        for unusable in [
            [f64::NAN, 40.0, 60.0],
            [f64::INFINITY, 40.0, 60.0],
            [-5.0, 45.0, 60.0],
            [20.0, 40.0, 140.0],
            [20.0, 40.0, 50.0],
            [10.0, 10.0, 10.0],
        ] {
            let layout = PanelLayouts {
                open: Some(unusable),
                collapsed: Some(unusable),
            };
            assert_eq!(layout.sanitised(), PanelLayouts::default(), "{unusable:?}");
        }
        // The centre decides which state a triple can belong to.
        let swapped = PanelLayouts {
            open: Some([25.0, 0.0, 75.0]),
            collapsed: Some([20.0, 40.0, 40.0]),
        };
        assert_eq!(swapped.sanitised(), PanelLayouts::default());
        // One bad triple does not take the good one with it.
        let mixed = PanelLayouts {
            open: Some([f64::NAN, 40.0, 60.0]),
            collapsed: Some([25.0, 0.0, 75.0]),
        };
        assert_eq!(mixed.sanitised().open, None);
        assert_eq!(mixed.sanitised().collapsed, Some([25.0, 0.0, 75.0]));
    }

    #[test]
    fn a_file_without_a_layout_loads_with_nothing_saved() {
        let settings: Settings =
            serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");

        assert_eq!(settings.layout, PanelLayouts::default());
        assert_eq!(Settings::default().layout, PanelLayouts::default());
    }

    #[test]
    fn a_layout_survives_a_round_trip_and_is_written_in_camel_case() {
        let settings = Settings {
            layout: PanelLayouts {
                open: Some([22.0, 38.0, 40.0]),
                collapsed: None,
            },
            ..Settings::default()
        };

        let text = serde_json::to_string(&settings).expect("serialises");
        let restored: Settings = serde_json::from_str(&text).expect("deserialises");

        assert_eq!(restored.layout, settings.layout);
        let json: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(json["layout"]["open"][0], 22.0);
        assert!(json["layout"]["collapsed"].is_null());
    }

    #[test]
    fn a_hand_edited_layout_never_makes_the_file_unreadable() {
        for body in [
            r#"{"layout":{"open":"wide"}}"#,
            r#"{"layout":{"open":[1,2]}}"#,
            r#"{"layout":{"open":[1,2,3,4]}}"#,
            r#"{"layout":{"open":[1,"x",98]}}"#,
            r#"{"layout":{"collapsed":null}}"#,
            r#"{"layout":{}}"#,
        ] {
            let settings: Settings = serde_json::from_str(body).expect(body);
            assert_eq!(
                settings.layout.sanitised(),
                PanelLayouts::default(),
                "{body}"
            );
        }
    }

    #[test]
    fn the_users_own_names_are_kept_clean_and_default_to_none() {
        assert!(Settings::default().own_names.is_empty());
        let typed = vec![
            "  Zorglub Metadatus ".to_string(),
            "".to_string(),
            "Zorglub Metadatus".to_string(),
            "Voirons Practice".to_string(),
        ];
        assert_eq!(
            clean_own_names(&typed),
            vec!["Zorglub Metadatus", "Voirons Practice"]
        );
        let older: Settings =
            serde_json::from_str(r#"{ "knowledgeMode": "auto" }"#).expect("reads");
        assert!(older.own_names.is_empty());
    }

    #[test]
    fn a_settings_file_written_before_the_names_setting_still_reads_and_hides_names() {
        // Settings are `serde(default)`: a file from an earlier build has no such key, and the
        // safe side (names hidden on a screen others can see) is the default.
        let older: Settings =
            serde_json::from_str(r#"{ "knowledgeMode": "auto" }"#).expect("reads");

        assert!(older.knowledge_hide_names_in_suggestions);
        assert_eq!(older.knowledge_mode, KnowledgeMode::Auto);
    }

    #[test]
    fn sources_are_not_copied_by_default_and_for_a_file_that_predates_the_setting() {
        assert!(!Settings::default().copy_sources);

        let older: Settings = serde_json::from_str(r#"{ "knowledgeMode": "auto" }"#).expect("reads");
        assert!(!older.copy_sources);
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
    fn the_targeting_settings_are_clamped_rather_than_trusted() {
        let wild = Settings {
            knowledge_min_scope_files: 0,
            knowledge_max_selectivity: 7.0,
            knowledge_min_reduction: -3.0,
            knowledge_max_candidates: 0,
            knowledge_widen_below_score: f32::NAN,
            ..Settings::default()
        };
        let policy = wild.targeting_policy();

        assert_eq!(policy.min_scope_files, 1);
        assert_eq!(policy.max_selectivity, 1.0);
        assert_eq!(policy.min_reduction, 0.0);
        assert_eq!(policy.max_candidates, 1);
        assert_eq!(
            policy.widen_below_score,
            DEFAULT_KNOWLEDGE_WIDEN_BELOW_SCORE
        );

        let defaults = Settings::default().targeting_policy();
        assert_eq!(defaults.min_scope_files, 8);
        assert_eq!(defaults.max_candidates, 15);
        assert!((defaults.max_selectivity - 0.6).abs() < 1e-6);
        assert!((defaults.min_reduction - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_file_written_before_the_targeting_settings_existed_still_loads() {
        let settings: Settings =
            serde_json::from_str(r#"{"knowledgeMode":"auto"}"#).expect("loads");

        assert_eq!(settings.knowledge_min_scope_files, 8);
        assert_eq!(settings.knowledge_max_candidates, 15);
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
        // Countries are cumulative (a cross-border worker): both stay, once each, in stored order.
        assert_eq!(odd.knowledge_pack_ids(), ["health-ch", "health-fr"]);

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
