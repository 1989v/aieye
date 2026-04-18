use crate::resume::{
    activate_app, find_running, focus_existing_tab, launch_in_terminal, resume_shell_command,
    HostApp, TerminalApp,
};
use crate::sessions::{CliKind, Session, SessionCoordinator};
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
    // 1. 세션이 지금 돌고 있나? (cwd 매칭 프로세스 존재)
    if let Some(cwd) = &session.project_path {
        let cli_name = match session.cli {
            CliKind::Claude => "claude",
            CliKind::Codex => "codex",
        };
        if let Some(running) = find_running(cli_name, cwd) {
            tracing::info!(
                "running session found: pid={} tty={} host={:?}",
                running.pid,
                running.tty,
                running.host_app
            );
            return match running.host_app {
                HostApp::Terminal => focus_existing_tab(TerminalApp::Terminal, &running.tty)
                    .map_err(|e| e.to_string()),
                HostApp::Iterm2 => focus_existing_tab(TerminalApp::Iterm2, &running.tty)
                    .map_err(|e| e.to_string()),
                HostApp::VsCode | HostApp::Jetbrains => {
                    // 실제 번들 이름 우선 (Cursor, WebStorm, PyCharm 등 정확히 activate)
                    if let Some(name) = running.host_app_name.as_deref() {
                        if activate_app(name).is_ok() {
                            return Ok(());
                        }
                    }
                    // 번들 이름 없거나 실패 시 enum 대표 이름 시도
                    if let Some(name) = running.host_app.app_name() {
                        if activate_app(name).is_ok() {
                            return Ok(());
                        }
                    }
                    // 최종 fallback: 새 터미널 런칭
                    launch_new(&session, terminal).map_err(|e| e.to_string())
                }
                HostApp::Other => launch_new(&session, terminal).map_err(|e| e.to_string()),
            };
        }
    }

    // 2. 실행 중이 아니면 새 터미널 런칭
    launch_new(&session, terminal).map_err(|e| e.to_string())
}

fn launch_new(session: &Session, terminal: Option<TerminalApp>) -> anyhow::Result<()> {
    let cmd = resume_shell_command(session);
    let term = terminal.unwrap_or_else(|| settings::load().preferred_terminal);
    tracing::info!(
        "launch_new: cli={:?} id={} term={:?}",
        session.cli,
        session.id,
        term
    );
    launch_in_terminal(term, &cmd)
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
