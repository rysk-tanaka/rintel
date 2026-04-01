mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::ai::check_ai_availability,
            commands::ai::send_message,
            commands::session::list_sessions,
            commands::session::create_session,
            commands::session::load_session,
            commands::session::delete_session,
            commands::session::cleanup_sessions,
            commands::file::add_file_context,
            commands::claude_code::list_claude_projects,
            commands::claude_code::list_claude_sessions,
            commands::claude_code::get_claude_session,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| eprintln!("tauri application error: {e}"));
}
