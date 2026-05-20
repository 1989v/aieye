use crate::resume::{
    activate_app, find_running, focus_existing_tab, launch_in_terminal, match_running,
    resume_shell_command, snapshot_running, HostApp, RunningInfo, TerminalApp,
};
use crate::parser::SessionPreview;
use crate::sessions::{CliKind, Session, SessionCoordinator, SessionPreviewInline};
use crate::settings::{self, Settings};
use crate::tray_state::SharedTrayState;
use std::path::PathBuf;
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
    // C: 행 1줄 요약 - 최근 20 세션만 (테일 256KB 읽기 비용 고려)
    for s in sessions.iter_mut().take(20) {
        let p = match s.cli {
            CliKind::Claude => crate::parser::claude_preview(&s.jsonl_path),
            CliKind::Codex => crate::parser::codex_preview(&s.jsonl_path),
        };
        s.inline_preview = Some(SessionPreviewInline {
            last_user: p.last_user,
            last_assistant: p.last_assistant,
        });
    }

    // repo_name: project_path 의 마지막 세그먼트
    for s in sessions.iter_mut() {
        s.repo_name = s
            .project_path
            .as_deref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from)
            .unwrap_or_else(|| "(no project)".to_string());
    }

    // active(running) 세션의 서브에이전트를 첫 응답에 동봉 (최대 20)
    let active_count = sessions
        .iter()
        .filter(|s| s.running.is_some() && matches!(s.cli, CliKind::Claude))
        .count()
        .min(20);
    let mut filled = 0usize;
    for s in sessions.iter_mut() {
        if filled >= active_count { break }
        if s.running.is_none() { continue }
        if !matches!(s.cli, CliKind::Claude) { continue }
        s.subagents = crate::parser::extract_subagents(&s.jsonl_path);
        filled += 1;
    }

    Ok(sessions)
}

/// B: hover 우측 패널용 최근 대화 턴.
#[tauri::command]
pub fn get_session_preview(jsonl_path: String, cli: CliKind) -> SessionPreview {
    let path = PathBuf::from(jsonl_path);
    match cli {
        CliKind::Claude => crate::parser::claude_preview(&path),
        CliKind::Codex => crate::parser::codex_preview(&path),
    }
}

/// 세션 jsonl 파일을 macOS 휴지통으로 이동 → 목록에서 제거.
/// 하드 삭제 대신 Finder 의 move-to-trash 사용 → 실수 시 복원 가능.
#[tauri::command]
pub fn archive_session_file(jsonl_path: String) -> Result<(), String> {
    move_to_trash(&jsonl_path)
}

fn move_to_trash(path: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err("file not found".into());
    }
    let script = format!(
        r#"tell application "Finder" to delete POSIX file "{}""#,
        path.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    tracing::info!("archived to trash: {path}");
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct BulkArchiveResult {
    pub archived: Vec<String>,
    pub skipped_recent: Vec<String>,
    pub errors: Vec<String>,
}

/// 여러 세션 파일을 일괄 휴지통 이동. 7일 이내 수정된 파일은 백엔드 안전장치로
/// 무조건 skip (프론트 필터와 이중 방어).
#[tauri::command]
pub fn archive_sessions_bulk(paths: Vec<String>) -> BulkArchiveResult {
    use std::time::{Duration, SystemTime};
    const SAFETY_WINDOW: Duration = Duration::from_secs(60 * 60 * 24 * 7);

    let mut result = BulkArchiveResult {
        archived: Vec::new(),
        skipped_recent: Vec::new(),
        errors: Vec::new(),
    };

    for path in paths {
        let p = PathBuf::from(&path);
        let md = match std::fs::metadata(&p) {
            Ok(m) => m,
            Err(e) => {
                result.errors.push(format!("{path}: {e}"));
                continue;
            }
        };
        let fresh = md
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .map(|d| d < SAFETY_WINDOW)
            .unwrap_or(false);
        if fresh {
            result.skipped_recent.push(path);
            continue;
        }
        match move_to_trash(&path) {
            Ok(()) => result.archived.push(path),
            Err(e) => result.errors.push(format!("{path}: {e}")),
        }
    }
    tracing::info!(
        "bulk archive: {} archived, {} skipped_recent, {} errors",
        result.archived.len(),
        result.skipped_recent.len(),
        result.errors.len()
    );
    result
}

#[tauri::command]
pub fn get_session_subagents(
    jsonl_path: String,
    cli: CliKind,
) -> Vec<crate::sessions::SubagentRow> {
    if !matches!(cli, CliKind::Claude) {
        return Vec::new();
    }
    crate::parser::extract_subagents(&PathBuf::from(jsonl_path))
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

#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_reply(session: Session, text: String) -> Result<(), String> {
    use crate::settings::ReplyMode;

    if text.trim().is_empty() {
        return Err("process_failed: empty message".into());
    }

    let cfg = crate::settings::load();
    let mode = cfg.reply_mode;

    // Headless 모드 가드: Claude only + idle only
    if matches!(mode, ReplyMode::Headless) {
        if !matches!(session.cli, CliKind::Claude) {
            return Err(
                "host_unsupported: Headless mode only supports Claude sessions.".into()
            );
        }
        let generating = session
            .running
            .as_ref()
            .and_then(|r| r.activity.as_ref())
            .map(|a| matches!(a, crate::parser::Activity::Generating))
            .unwrap_or(false);
        if generating {
            return Err(
                "session_running: Session is generating. Switch to paste mode or wait.".into()
            );
        }
        return crate::resume::headless::send(&session, &text).await;
    }

    // Paste 모드
    crate::resume::paste::send(&session, &text).await
}
