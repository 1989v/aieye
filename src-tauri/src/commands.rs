use crate::resume::{launch_in_terminal, resume_shell_command, TerminalApp};
use crate::sessions::{Session, SessionCoordinator};
use crate::settings::{self, Settings};

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<Session>, String> {
    let coord = SessionCoordinator::with_defaults();
    Ok(coord.scan_all().await)
}

#[tauri::command]
pub async fn resume_session(
    session: Session,
    terminal: Option<TerminalApp>,
) -> Result<(), String> {
    let cmd = resume_shell_command(&session);
    let term = terminal.unwrap_or_else(|| settings::load().preferred_terminal);
    tracing::info!("resume_session: cli={:?} id={} term={:?} cmd={}", session.cli, session.id, term, cmd);
    match launch_in_terminal(term, &cmd) {
        Ok(()) => {
            tracing::info!("launch_in_terminal ok");
            Ok(())
        }
        Err(e) => {
            tracing::error!("launch_in_terminal failed: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .args(["-R", &path])
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_installed_terminals() -> Vec<TerminalApp> {
    TerminalApp::all()
        .iter()
        .copied()
        .filter(|t| t.is_installed())
        .collect()
}

#[tauri::command]
pub fn get_settings() -> Settings {
    settings::load()
}

#[tauri::command]
pub fn set_settings(settings: Settings) -> Result<(), String> {
    crate::settings::save(&settings).map_err(|e| e.to_string())
}
