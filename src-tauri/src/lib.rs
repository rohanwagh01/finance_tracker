mod commands;
mod db;
mod error;
mod http;
mod investments;
mod providers;
mod research;
mod rules;
mod secrets;
mod state;
mod sync;
mod util;
mod vault;

use tauri::Manager;

use crate::state::{AppState, VaultPaths};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("resolve app data dir");
            std::fs::create_dir_all(&data_dir).ok();

            // The database is opened lazily by `vault_unlock` / `vault_initialize`
            // once the master password is known — nothing private is readable
            // before that.
            let paths = VaultPaths {
                meta: data_dir.join("vault.meta"),
                db: data_dir.join("vault.db"),
            };
            app.manage(AppState::new(paths));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::vault::vault_status,
            commands::vault::vault_initialize,
            commands::vault::vault_unlock,
            commands::vault::vault_lock,
            commands::vault::vault_change_password,
            commands::vault::vault_reset,
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
            commands::accounts::plaid_link_start,
            commands::accounts::plaid_link_poll,
            commands::accounts::list_items,
            commands::accounts::list_accounts,
            commands::accounts::sync_item,
            commands::accounts::sync_all,
            commands::accounts::unlink_item,
            commands::accounts::set_account_shared,
            commands::accounts::set_account_hidden,
            commands::accounts::reset_account_attributions,
            commands::spending::spending_summary,
            commands::spending::spending_trends,
            commands::spending::spending_by_person,
            commands::spending::spending_children,
            commands::spending::list_transactions,
            commands::spending::account_transactions,
            commands::spending::list_categories,
            commands::spending::set_transaction_category,
            commands::review::review_inbox,
            commands::review::review_count,
            commands::review::review_decide,
            commands::review::review_reopen,
            commands::review::list_rules,
            commands::review::create_rule,
            commands::review::update_rule,
            commands::review::delete_rule,
            commands::review::apply_rules_now,
            commands::investments::portfolio,
            commands::investments::portfolio_history,
            commands::investments::account_value_history,
            commands::networth::net_worth_history,
            commands::networth::net_worth_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
