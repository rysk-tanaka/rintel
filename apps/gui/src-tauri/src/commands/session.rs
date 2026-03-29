use ai_session::{Session, SessionConfig};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: Option<String>,
    pub last_active: DateTime<Utc>,
    pub message_count: usize,
    pub expired: bool,
}

#[derive(Serialize)]
pub struct SessionDetailDto {
    pub id: String,
    pub title: Option<String>,
    pub system_prompt: Option<String>,
    pub messages: Vec<ai_provider::types::Message>,
    pub file_contexts: Vec<ai_provider::types::FileContext>,
}

#[derive(Serialize)]
pub struct SessionInfoDto {
    pub id: String,
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummaryDto>, String> {
    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    let sessions = manager.list().map_err(|e| e.to_string())?;
    Ok(sessions
        .into_iter()
        .map(|s| SessionSummaryDto {
            id: s.id.to_string(),
            title: s.title,
            last_active: s.last_active,
            message_count: s.message_count,
            expired: s.expired,
        })
        .collect())
}

#[tauri::command]
pub fn create_session(
    system_prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<SessionInfoDto, String> {
    let config = SessionConfig::default();
    let ttl_secs = config.default_ttl.map(|d| d.as_secs());
    let session = Session::new(system_prompt, ttl_secs);
    let id = session.id.to_string();

    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    manager.save(&session).map_err(|e| e.to_string())?;

    Ok(SessionInfoDto { id })
}

#[tauri::command]
pub fn load_session(id: String, state: State<'_, AppState>) -> Result<SessionDetailDto, String> {
    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    let uuid = manager.resolve_prefix(&id).map_err(|e| e.to_string())?;
    let session = manager.load(&uuid).map_err(|e| e.to_string())?;

    Ok(SessionDetailDto {
        id: session.id.to_string(),
        title: session.title,
        system_prompt: session.system_prompt,
        messages: session.messages,
        file_contexts: session.file_contexts,
    })
}

#[tauri::command]
pub fn delete_session(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    let uuid = manager.resolve_prefix(&id).map_err(|e| e.to_string())?;
    manager.delete(&uuid).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cleanup_sessions(state: State<'_, AppState>) -> Result<usize, String> {
    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    manager.cleanup_expired().map_err(|e| e.to_string())
}
