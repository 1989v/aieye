use crate::sessions::{ClaudeAdapter, Session, SessionAdapter};

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<Session>, String> {
    let adapter = ClaudeAdapter::with_defaults();
    adapter.scan().await.map_err(|e| e.to_string())
}
