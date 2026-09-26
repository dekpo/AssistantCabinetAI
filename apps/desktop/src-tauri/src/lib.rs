//! Assistant Cabinet AI, the practice window.
//!
//! Rust does the native work only: settings, the work folder allow-list, and talking to the
//! gateway. The interface is React and TypeScript, and it reaches none of this except through the
//! commands registered below.

pub mod analysis_scope;
pub mod cancellation;
pub mod chunking;
mod commands;
pub mod discovery;
pub mod error;
pub mod extraction;
pub mod file_record;
pub mod file_reference;
pub mod filename_sanitizer;
pub mod folder_questions;
pub mod gateway;
pub mod index_store;
pub mod indexing;
pub mod inventory;
pub mod ocr;
pub mod raster;
pub mod retrieval;
pub mod reveal;
mod settings;
mod work_folder;
pub mod work_folder_context;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new().expect("the outbound HTTP client must build");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::load_app_snapshot,
            commands::save_settings,
            commands::reset_settings,
            commands::choose_work_folder,
            commands::ensure_suggested_work_folder,
            commands::check_server_health,
            commands::send_chat_message,
            commands::cancel_chat,
            commands::index_work_folder,
            commands::ask_with_sources,
            commands::has_indexed_documents,
            commands::work_folder_inventory,
            commands::reveal_work_folder,
            commands::reveal_work_file,
            commands::reset_index,
        ])
        .run(tauri::generate_context!())
        .expect("the application must start");
}
