//! AETHER-OS desktop entry point.
//!
//! Wires together: Tauri runtime, plugin allowlist, IPC commands,
//! and the Rust core crates that implement the seven architectural pillars.

mod commands;
mod state;

use tauri::Manager;

/// Main entry. Called from `main.rs` and from mobile entry points (future).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    aether_telemetry::init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();
            tokio::spawn(async move {
                if let Err(e) = state::AppState::initialize(&handle).await {
                    tracing::error!(error = %e, "failed to initialize AppState");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_app_info,
            commands::system::get_runtime_status,
            commands::auth::passkey_register_start,
            commands::auth::passkey_register_finish,
            commands::auth::passkey_login_start,
            commands::auth::passkey_login_finish,
            commands::auth::session_lock,
            commands::sync::push_outbox,
            commands::sync::pull_changes,
            commands::sync::sync_status,
            commands::protocols::list_bridges,
            commands::protocols::subscribe_tag,
            commands::protocols::unsubscribe_tag,
            commands::modules::list_installed,
            commands::modules::install_module,
            commands::modules::uninstall_module,
            commands::safety::trigger_sos,
            commands::safety::geofence_evaluate,
            commands::discovery::discovery_scan,
            commands::discovery::discovery_save_device,
            commands::healing::healing_diagnose,
            commands::healing::healing_apply,
            commands::healing::healing_recent_events,
            commands::interlock::interlock_request_unlock,
            commands::interlock::interlock_user_certifications,
            commands::interlock::interlock_machine_required_certs,
            commands::hedging::hedging_recommendations,
            commands::hedging::hedging_commodity_prices,
            commands::i18n::i18n_locales,
            commands::i18n::i18n_set_locale,
            commands::i18n::i18n_format_money,
            commands::industry::industry_list,
            commands::industry::industry_profile,
            commands::industry::industry_set,
            commands::compliance::compliance_standards,
            commands::compliance::compliance_run,
            commands::compliance::compliance_latest_report,
            commands::healing::healing_manager_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AETHER-OS");
}
