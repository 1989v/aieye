//! 현재 실행 중인 claude / codex 프로세스를 찾아 해당 세션이 어느 터미널에
//! 붙어있는지 판별.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 매칭된 실행중 세션 정보.
#[derive(Debug, Clone)]
pub struct RunningSession {
    pub pid: u32,
    pub tty: String,
    pub host_app: HostApp,
    /// 실제 호스트 앱 bundle 이름 (e.g. "WebStorm", "Cursor").
    /// classify 는 enum 으로 카테고리화하지만 activate 는 정확한 이름이 필요.
    pub host_app_name: Option<String>,
    /// 프로세스의 정규화된 cwd — snapshot → 세션 매칭 용도.
    #[allow(dead_code)]
    pub cwd: PathBuf,
    /// 프로세스가 열고 있는 jsonl 에서 추출한 세션 ID (claude/codex).
    /// 같은 cwd 에 세션이 여러 개일 때 행을 정확히 1개만 태깅하기 위함.
    pub session_id: Option<String>,
}

/// 프론트엔드로 노출되는 요약형.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningInfo {
    pub pid: u32,
    pub tty: String,
    pub host_kind: String,
    pub host_name: Option<String>,
    /// jsonl 마지막 턴 기반 활성 상태 — enrich 단계에서 별도 주입.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<crate::parser::Activity>,
}

impl From<&RunningSession> for RunningInfo {
    fn from(s: &RunningSession) -> Self {
        let host_kind = match s.host_app {
            HostApp::Terminal => "terminal",
            HostApp::Iterm2 => "iterm2",
            HostApp::VsCode => "vscode",
            HostApp::Jetbrains => "jetbrains",
            HostApp::Other => "other",
        };
        RunningInfo {
            pid: s.pid,
            tty: s.tty.clone(),
            host_kind: host_kind.to_string(),
            host_name: s.host_app_name.clone(),
            activity: None,
        }
    }
}

/// 터미널/IDE 구분.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostApp {
    Terminal,
    Iterm2,
    /// VS Code 내부 터미널
    VsCode,
    /// JetBrains (IntelliJ, Fleet, etc.)
    Jetbrains,
    Other,
}

impl HostApp {
    pub fn app_name(self) -> Option<&'static str> {
        match self {
            HostApp::Terminal => Some("Terminal"),
            HostApp::Iterm2 => Some("iTerm"),
            HostApp::VsCode => Some("Code"),
            HostApp::Jetbrains => Some("IntelliJ IDEA"),
            HostApp::Other => None,
        }
    }

    pub fn can_focus_tab(self) -> bool {
        matches!(self, HostApp::Terminal | HostApp::Iterm2)
    }
}

/// 한 cli 의 모든 실행 중 후보를 미리 수집. 대량 세션 enrich 시 pgrep/lsof
/// 호출을 세션 수가 아닌 후보 프로세스 수로 제한한다.
pub fn snapshot_running(cli_name: &str) -> Vec<RunningSession> {
    let pids = pgrep(cli_name).unwrap_or_default();
    tracing::info!(
        "snapshot_running: cli={} candidates={:?}",
        cli_name,
        pids
    );
    let mut out = Vec::new();
    for pid in pids {
        let Some(proc_cwd) = get_process_cwd(pid) else {
            continue;
        };
        let Some(tty) = get_process_tty(pid) else {
            continue;
        };
        let (host_app, host_app_name) = detect_host_app(pid);
        let session_id = detect_session_id(pid, cli_name);
        out.push(RunningSession {
            pid,
            tty,
            host_app,
            host_app_name,
            cwd: std::fs::canonicalize(&proc_cwd).unwrap_or(proc_cwd),
            session_id,
        });
    }
    out
}

/// 프로세스 argv 에서 `--resume <id>` 를 찾아 세션 ID 추출.
/// claude 는 jsonl 을 연속 open 하지 않아 lsof 로 못 찾음.
/// - `claude --resume 5a34...` → Some("5a34...")
/// - `claude` (새 세션) → None → match 단계에서 mtime fallback
fn detect_session_id(pid: u32, _cli_name: &str) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let cmd = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        if t == "--resume" || t == "resume" {
            if let Some(next) = tokens.get(i + 1) {
                if !next.starts_with('-') {
                    return Some(next.to_string());
                }
            }
        } else if let Some(rest) = t.strip_prefix("--resume=") {
            return Some(rest.to_string());
        }
        i += 1;
    }
    None
}

/// codex rollout stem → 마지막 5개 hyphen-block (UUID v7) 추출.
fn extract_codex_id(stem: &str) -> String {
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 5 {
        parts[parts.len() - 5..].join("-")
    } else {
        stem.to_string()
    }
}

/// snapshot 결과에서 주어진 cwd + session_id 에 매칭되는 세션을 찾음.
/// session_id 가 프로세스 argv 에 있으면 정확 매칭. 없으면 cwd 만 매칭.
/// (cwd 충돌은 commands.rs 에서 most-recent 세션에만 태깅하는 로직으로 해결)
pub fn match_running<'a>(
    snapshot: &'a [RunningSession],
    cwd: &Path,
    session_id: &str,
) -> Option<&'a RunningSession> {
    let target = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    // 1) 정확 매칭 (--resume 인자가 세션 ID 와 일치)
    if let Some(r) = snapshot
        .iter()
        .find(|s| s.cwd == target && s.session_id.as_deref() == Some(session_id))
    {
        return Some(r);
    }
    // 2) fallback: cwd + session_id None (newly started claude, 가장 최근 세션일 확률 큼)
    snapshot
        .iter()
        .find(|s| s.cwd == target && s.session_id.is_none())
}

/// 주어진 cli (claude/codex) 와 cwd 에 매칭되는 실행 중 프로세스를 찾는다.
pub fn find_running(cli_name: &str, cwd: &Path) -> Option<RunningSession> {
    let pids = pgrep(cli_name).unwrap_or_default();
    tracing::info!(
        "find_running: cli={} cwd={} candidates={:?}",
        cli_name,
        cwd.display(),
        pids
    );
    if pids.is_empty() {
        return None;
    }
    // 심볼릭 링크 정규화 (예: /tmp → /private/tmp)
    let target = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    for pid in pids {
        let proc_cwd = match get_process_cwd(pid) {
            Some(p) => p,
            None => {
                tracing::info!("  pid={} cwd=<none>", pid);
                continue;
            }
        };
        let proc_cwd_canon =
            std::fs::canonicalize(&proc_cwd).unwrap_or_else(|_| proc_cwd.clone());
        let matches = proc_cwd_canon == target;
        tracing::info!(
            "  pid={} cwd={} match={}",
            pid,
            proc_cwd.display(),
            matches
        );
        if matches {
            let tty = get_process_tty(pid);
            let (host_app, host_app_name) = detect_host_app(pid);
            tracing::info!(
                "  → matched: tty={:?} host_app={:?} bundle={:?}",
                tty,
                host_app,
                host_app_name
            );
            // tty 없는 후보는 bg 스크립트 등 — 다음 후보로 넘어감
            let Some(tty) = tty else { continue };
            let session_id = detect_session_id(pid, cli_name);
            return Some(RunningSession {
                pid,
                tty,
                host_app,
                host_app_name,
                cwd: proc_cwd_canon,
                session_id,
            });
        }
    }
    tracing::info!("find_running: no match for cwd={}", target.display());
    None
}

fn pgrep(name: &str) -> Option<Vec<u32>> {
    // 1차: -x 로 argv[0] 이 정확히 name 인 프로세스만 (bg 스크립트의 오탐 방지)
    let mut pids = run_pgrep(&["-x", name]);
    // 2차 fallback: -x 로 못 찾으면 node 래퍼 가능성 → `/claude` 나 `/codex` 로 끝나는 경로만 매칭
    if pids.is_empty() {
        let pattern = format!("/{name}$");
        pids = run_pgrep(&["-f", &pattern]);
    }
    if pids.is_empty() {
        None
    } else {
        Some(pids)
    }
}

fn run_pgrep(args: &[&str]) -> Vec<u32> {
    let output = match Command::new("pgrep").args(args).output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let self_pid = std::process::id();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|s| s.trim().parse::<u32>().ok())
        .filter(|pid| *pid != self_pid)
        .collect()
}

fn get_process_cwd(pid: u32) -> Option<PathBuf> {
    // macOS: lsof -a -p PID -d cwd -Fn → 'n<path>' 라인
    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(stripped) = line.strip_prefix('n') {
            if stripped.starts_with('/') {
                return Some(PathBuf::from(stripped));
            }
        }
    }
    None
}

fn get_process_tty(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tty.is_empty() || tty == "??" {
        None
    } else {
        Some(format!("/dev/{tty}"))
    }
}

fn get_ppid(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()
}

fn get_process_name(pid: u32) -> Option<String> {
    // -o command= 은 전체 경로 포함 → 판별 쉬움
    let output = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 프로세스 command 경로에서 `.app` 번들 이름을 추출.
/// "/Applications/WebStorm.app/Contents/MacOS/webstorm" → Some("WebStorm")
pub fn extract_app_bundle_name(command: &str) -> Option<String> {
    let idx = command.find(".app/")?;
    let before_app = &command[..idx];
    let start = before_app.rfind('/').map(|i| i + 1).unwrap_or(0);
    Some(before_app[start..].to_string())
}

fn classify_app(command: &str) -> HostApp {
    let lower = command.to_lowercase();
    if lower.contains("/terminal.app/") {
        HostApp::Terminal
    } else if lower.contains("/iterm.app/") {
        HostApp::Iterm2
    } else if lower.contains("/visual studio code.app/")
        || lower.contains("/code - insiders.app/")
        || lower.contains("/cursor.app/")
        || lower.contains("/code helper")
    {
        HostApp::VsCode
    } else if lower.contains("/intellij idea") || lower.contains("/idea")
        || lower.contains("/jetbrains")
        || lower.contains("/webstorm") || lower.contains("/pycharm")
        || lower.contains("/goland") || lower.contains("/rubymine")
        || lower.contains("/rider") || lower.contains("/clion")
        || lower.contains("/phpstorm") || lower.contains("/datagrip")
        || lower.contains("/appcode") || lower.contains("/fleet.app")
        || lower.contains("/android studio.app")
    {
        HostApp::Jetbrains
    } else {
        HostApp::Other
    }
}

/// pid 의 부모 체인을 따라 올라가며 호스트 앱 분류 + 정확한 번들 이름 추출.
fn detect_host_app(pid: u32) -> (HostApp, Option<String>) {
    let mut current = pid;
    for _ in 0..12 {
        let Some(ppid) = get_ppid(current) else {
            return (HostApp::Other, None);
        };
        if ppid <= 1 {
            return (HostApp::Other, None);
        }
        if let Some(name) = get_process_name(ppid) {
            let kind = classify_app(&name);
            if kind != HostApp::Other {
                return (kind, extract_app_bundle_name(&name));
            }
        }
        current = ppid;
    }
    (HostApp::Other, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_terminal_paths() {
        assert_eq!(
            classify_app("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"),
            HostApp::Terminal
        );
        assert_eq!(
            classify_app("/Applications/iTerm.app/Contents/MacOS/iTerm2"),
            HostApp::Iterm2
        );
        assert_eq!(
            classify_app("/Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
            HostApp::VsCode
        );
        assert_eq!(
            classify_app(
                "/Applications/IntelliJ IDEA.app/Contents/MacOS/idea"
            ),
            HostApp::Jetbrains
        );
        assert_eq!(classify_app("/bin/zsh"), HostApp::Other);
    }
}
