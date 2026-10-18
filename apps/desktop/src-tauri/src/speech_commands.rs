//! The Tauri shell of the speech module: its own managed state and thin commands over the pure
//! functions of `speech/`. It touches neither `AppState`'s fields nor `commands.rs`, only
//! `AppState.settings`, which is public.
//!
//! Nothing here runs at launch: `SpeechState::new` builds empty values, and a command runs only
//! when the interface asks.

use std::sync::Mutex;

use tauri::State;

use crate::commands::AppState;
use crate::error::AppError;
use crate::speech::status::{self, Probes, SpeechStatus};

/// What the speech module keeps between two commands. Later phases add the running job and the
/// worker handles here; for now it holds what was found on this machine, which is nothing.
#[derive(Default)]
pub struct SpeechState {
    probes: Mutex<Probes>,
}

impl SpeechState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn probes(&self) -> Result<Probes, AppError> {
        self.probes
            .lock()
            .map(|probes| *probes)
            .map_err(|_| AppError::Internal)
    }
}

/// Per feature (dictation, read-aloud, notes): can it run, and if not the machine code of the
/// first reason; plus the language table. Reads the settings and the probes, starts nothing.
#[tauri::command]
pub fn speech_status(
    state: State<'_, AppState>,
    speech: State<'_, SpeechState>,
) -> Result<SpeechStatus, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal)?
        .clone();
    Ok(status::compute(&settings, &speech.probes()?))
}
