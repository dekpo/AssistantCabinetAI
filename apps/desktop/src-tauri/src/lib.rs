//! Assistant Cabinet AI, the practice window.
//!
//! Rust does the native work only: settings, the work folder allow-list, and talking to the
//! gateway. The interface is React and TypeScript, and it reaches none of this except through the
//! commands registered below.

pub mod analysis_scope;
pub mod analysis_stop;
pub mod cancellation;
pub mod cards;
pub mod chunking;
pub mod chunking_long;
pub mod cleaning;
mod commands;
pub mod conversation;
pub mod data_folder;
pub mod discovery;
pub mod error;
pub mod extraction;
pub mod family_view;
pub mod file_key;
pub mod file_profile;
pub mod file_record;
pub mod file_reference;
pub mod fill_plan;
pub mod filename_sanitizer;
pub mod finder;
pub mod folder_questions;
pub mod gateway;
pub mod index_store;
pub mod indexing;
pub mod inventory;
pub mod knowledge;
pub mod mixed_answer;
pub mod number_check;
pub mod ocr;
pub mod ocr_resume;
pub mod raster;
pub mod readers;
pub mod retrieval;
pub mod reveal;
mod settings;
pub mod speech;
mod speech_commands;
pub mod structure;
pub mod tabular;
pub mod tabular_answer;
pub mod template_fill;
pub mod viewer_source;
mod work_folder;
pub mod work_folder_context;
pub mod workbook_use;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new().expect("the outbound HTTP client must build");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state)
        .manage(speech_commands::SpeechState::new())
        .invoke_handler(tauri::generate_handler![
            commands::load_app_snapshot,
            commands::save_settings,
            commands::reset_settings,
            commands::choose_work_folder,
            commands::ensure_suggested_work_folder,
            commands::choose_data_folder,
            commands::ensure_suggested_data_folder,
            commands::reveal_data_folder,
            commands::reveal_data_file,
            commands::read_data_file_for_viewer,
            commands::read_data_file_readable_text,
            commands::index_data_folder,
            commands::data_folder_inventory,
            commands::reset_data_index,
            commands::check_server_health,
            commands::cancel_chat,
            commands::index_work_folder,
            commands::ask_with_sources,
            commands::knowledge_diagnostics,
            commands::knowledge_search_entities,
            commands::knowledge_suggest_questions,
            commands::knowledge_entity_card,
            commands::speech_repair_names,
            commands::knowledge_overview,
            commands::knowledge_list_entities,
            commands::knowledge_entity_sheet,
            commands::knowledge_possible_matches,
            commands::knowledge_health,
            commands::knowledge_types_and_roles,
            commands::knowledge_create_entity,
            commands::knowledge_edit_entity,
            commands::knowledge_add_alias,
            commands::knowledge_remove_alias,
            commands::knowledge_set_attribute,
            commands::knowledge_remove_attribute,
            commands::knowledge_merge,
            commands::knowledge_unmerge,
            commands::knowledge_split,
            commands::knowledge_delete_entity,
            commands::knowledge_restore_entity,
            commands::knowledge_accept_match,
            commands::knowledge_dismiss_match,
            commands::knowledge_undo,
            commands::knowledge_reset,
            commands::has_indexed_documents,
            commands::profile_index_status,
            commands::cancel_analysis,
            commands::vector_share,
            commands::find_files,
            commands::file_knowledge_facts,
            commands::work_folder_inventory,
            commands::reveal_work_folder,
            commands::reveal_work_file,
            commands::read_work_file_for_viewer,
            commands::read_work_file_readable_text,
            commands::reset_index,
            commands::fill_preview,
            commands::fill_generate,
            speech_commands::speech_status,
            speech_commands::notes_preview,
            speech_commands::notes_save,
            speech_commands::speech_transcribe,
            speech_commands::speech_cancel,
            speech_commands::speech_model_install,
            speech_commands::speech_model_verify,
            speech_commands::speech_model_cancel,
            speech_commands::tts_synthesize,
            speech_commands::tts_cancel,
            speech_commands::speech_prepare_reading,
            speech_commands::speech_apply_dictation_commands,
        ])
        .run(tauri::generate_context!())
        .expect("the application must start");
}
