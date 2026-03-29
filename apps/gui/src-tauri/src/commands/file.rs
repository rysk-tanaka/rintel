use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct FileContextInfoDto {
    pub filename: String,
    pub size: usize,
}

#[tauri::command]
pub fn add_file_context(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<FileContextInfoDto, String> {
    let manager = state.session_manager.lock().map_err(|e| e.to_string())?;
    let uuid = manager
        .resolve_prefix(&session_id)
        .map_err(|e| e.to_string())?;
    let mut session = manager.load(&uuid).map_err(|e| e.to_string())?;

    session
        .add_file_context(Path::new(&path))
        .map_err(|e| e.to_string())?;

    let added = session
        .file_contexts
        .last()
        .map(|fc| FileContextInfoDto {
            filename: fc.filename.clone(),
            size: fc.content.len(),
        })
        .ok_or_else(|| "failed to add file context".to_string())?;

    manager.save(&session).map_err(|e| e.to_string())?;

    Ok(added)
}
