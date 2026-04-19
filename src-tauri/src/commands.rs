use crate::resume::{
    activate_app, find_running, focus_existing_tab, launch_in_terminal, match_running,
    resume_shell_command, snapshot_running, HostApp, RunningInfo, TerminalApp,
};
use crate::sessions::{CliKind, Session, SessionCoordinator};
use crate::settings::{self, Settings};
use crate::tray_state::SharedTrayState;
use tauri::State;

#[tauri::command]
pub async fn list_sessions(state: State<'_, SharedTrayState>) -> Result<Vec<Session>, String> {
    let coord = SessionCoordinator::with_defaults();
    let mut sessions = coord.scan_all().await;
    let claude_snap = snapshot_running("claude");
    let codex_snap = snapshot_running("codex");
    // session_id 매칭이 None 인 running 프로세스는 1개만 최근 세션에 태깅 (cwd 충돌 방지)
    let mut tagged_pids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for s in &mut sessions {
        let Some(cwd) = s.project_path.as_deref() else { continue };
        let snap = match s.cli {
            CliKind::Claude => &claude_snap,
            CliKind::Codex => &codex_snap,
        };
        let r = match_running(snap, cwd, &s.id);
        let Some(r) = r else { continue };
        if tagged_pids.contains(&r.pid) {
            continue;
        }
        tagged_pids.insert(r.pid);
        let mut info = RunningInfo::from(r);
        info.activity = Some(match s.cli {
            CliKind::Claude => crate::parser::claude_activity(&s.jsonl_path),
            CliKind::Codex => crate::parser::codex_activity(&s.jsonl_path),
        });
        s.running = Some(info);
    }
    if let Ok(ts) = state.0.lock() {
        for s in &mut sessions {
            s.finished = ts.is_finished(&s.id);
        }
    }
    Ok(sessions)
}

#[tauri::command]
pub fn acknowledge_finished(session_id: String, state: State<'_, SharedTrayState>) {
    if let Ok(mut ts) = state.0.lock() {
        ts.acknowledge(&session_id);
    }
}

fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    let Ok(mut child) = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    else {
        return;
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
}

#[tauri::command]
pub async fn resume_session(
    session: Session,
    terminal: Option<TerminalApp>,
    state: State<'_, SharedTrayState>,
) -> Result<(), String> {
    if let Ok(mut ts) = state.0.lock() {
        ts.acknowledge(&session.id);
    }
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
                    // IDE 내부 터미널 탭은 AppleScript 로 선택 불가 → tty 를
                    // 클립보드에 복사해서 사용자가 직접 탭 찾기 쉽게 함.
                    copy_to_clipboard(&running.tty);
                    tracing::info!("tty {} copied to clipboard", running.tty);
                    if let Some(name) = running.host_app_name.as_deref() {
                        if activate_app(name).is_ok() {
                            return Ok(());
                        }
                    }
                    if let Some(name) = running.host_app.app_name() {
                        if activate_app(name).is_ok() {
                            return Ok(());
                        }
                    }
                    launch_new(&session, terminal).map_err(|e| e.to_string())
                }
                HostApp::Other => launch_new(&session, terminal).map_err(|e| e.to_string()),
            };
        }
    }

    // 2. 실행 중이 아니면 새 터미널 런칭
    launch_new(&session, terminal).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_session_force_new(
    session: Session,
    terminal: Option<TerminalApp>,
    state: State<'_, SharedTrayState>,
) -> Result<(), String> {
    if let Ok(mut ts) = state.0.lock() {
        ts.acknowledge(&session.id);
    }
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
