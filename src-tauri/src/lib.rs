pub mod commands;
pub mod db;
pub mod error;
pub mod llm;
pub mod models;
pub mod parser;
pub mod paths;
pub mod pipeline;
pub mod state;

use db::Db;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            // 数据存于用户目录（例如 Linux: ~/.config/local/chronospace-reader/）。
            paths::ensure_data_dir().expect("无法创建应用数据目录");
            let db = Db::open(&paths::db_path()).expect("初始化 SQLite 失败");
            app.manage(AppState::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::inspect_book,
            commands::ingest_book,
            commands::list_books,
            commands::get_timeline,
            commands::get_books_timeline,
            commands::delete_book,
            commands::clear_all_data,
            commands::set_book_source,
            commands::get_book_source,
            commands::save_llm_config,
            commands::get_llm_config,
            commands::list_models,
            commands::list_loaded_models,
            commands::activate_model,
            commands::test_connection,
            commands::gpu_status,
            commands::start_extraction,
            commands::pause_extraction,
            commands::resume_extraction,
            commands::cancel_extraction,
            commands::read_text_file,
            commands::get_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
