mod commands;
mod db;
mod error;
mod llm;
mod models;
mod parser;
mod pipeline;
mod state;

use db::{default_db_path, Db};
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("无法解析应用数据目录");
            let db = Db::open(&default_db_path(&data_dir)).expect("初始化 SQLite 失败");
            app.manage(AppState::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ingest_book,
            commands::list_books,
            commands::get_timeline,
            commands::get_books_timeline,
            commands::delete_book,
            commands::save_llm_config,
            commands::get_llm_config,
            commands::list_models,
            commands::test_connection,
            commands::start_extraction,
            commands::pause_extraction,
            commands::resume_extraction,
            commands::cancel_extraction,
            commands::read_text_file,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}
