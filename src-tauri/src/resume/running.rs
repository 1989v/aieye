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
}

/// 프론트엔드로 노출되는 요약형.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningInfo {
    pub pid: u32,
    pub tty: String,
    pub host_kind: String,
    pub host_name: Option<String>,
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
            return Some(RunningSession {
                pid,
                tty,
                host_app,
                host_app_name,
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
