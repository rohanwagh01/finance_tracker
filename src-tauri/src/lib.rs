mod commands;
mod db;
mod error;
mod http;
mod providers;
mod research;
mod secrets;
mod state;
mod util;

use tauri::Manager;

use crate::db::Db;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("resolve app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("finance_tracker.sqlite3");

            let db = tauri::async_runtime::block_on(Db::connect(&db_path))
                .expect("open database and run migrations");

            app.manage(AppState::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_setup_status,
            commands::settings::list_credentials,
            commands::settings::save_credential,
            commands::settings::delete_credential,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::test_plaid_connection,
            commands::settings::complete_onboarding,
            commands::people::list_people,
            commands::people::create_person,
            commands::people::update_person,
            commands::people::delete_person,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
