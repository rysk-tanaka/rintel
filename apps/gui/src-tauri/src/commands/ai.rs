use ai_provider::provider::AiProvider;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct SendMessageResponse {
    pub content: String,
    pub messages: Vec<ai_provider::types::Message>,
}

#[tauri::command]
pub fn check_ai_availability(state: State<'_, AppState>) -> bool {
    state.provider.is_available()
}

#[tauri::command]
pub async fn send_message(
    session_id: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<SendMessageResponse, String> {
    let (session, provider) = {
        let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
        let id = manager
            .resolve_prefix(&session_id)
            .map_err(|e| e.to_string())?;
        let session = manager.load(&id).map_err(|e| e.to_string())?;
        (session, state.provider.clone())
    };

    let (content, session) = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.send(&provider, &message);
        result.map(|content| (content, session))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let messages = session.messages.clone();

    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    manager.save(&session).map_err(|e| e.to_string())?;

    Ok(SendMessageResponse { content, messages })
}
