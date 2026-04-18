use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalApp {
    Terminal,
    Iterm2,
    Warp,
    Alacritty,
    Kitty,
}

impl TerminalApp {
    pub fn all() -> &'static [TerminalApp] {
        &[
            TerminalApp::Terminal,
            TerminalApp::Iterm2,
            TerminalApp::Warp,
            TerminalApp::Alacritty,
            TerminalApp::Kitty,
        ]
    }

    pub fn bundle_id(self) -> &'static str {
        match self {
            TerminalApp::Terminal => "com.apple.Terminal",
            TerminalApp::Iterm2 => "com.googlecode.iterm2",
            TerminalApp::Warp => "dev.warp.Warp-Stable",
            TerminalApp::Alacritty => "org.alacritty",
            TerminalApp::Kitty => "net.kovidgoyal.kitty",
        }
    }

    /// mdfind 로 bundle id 매칭되는 .app 이 시스템에 존재하는지 확인.
    /// macOS 는 Terminal 은 기본 내장이지만 확인 결과에도 포함됨.
    pub fn is_installed(self) -> bool {
        let q = format!("kMDItemCFBundleIdentifier == '{}'", self.bundle_id());
        std::process::Command::new("mdfind")
            .args(["-onlyin", "/Applications", &q])
            .output()
            .ok()
            .map(|out| !out.stdout.is_empty())
            .unwrap_or(false)
            || {
                // macOS 시스템 Terminal.app 폴백
                matches!(self, TerminalApp::Terminal)
                    && std::path::Path::new("/System/Applications/Utilities/Terminal.app").exists()
            }
    }
}

pub fn launch_in_terminal(app: TerminalApp, shell_command: &str) -> anyhow::Result<()> {
    match app {
        TerminalApp::Terminal => launch_terminal_app(shell_command),
        TerminalApp::Iterm2 => launch_iterm2(shell_command),
        TerminalApp::Warp => launch_warp(shell_command),
        TerminalApp::Alacritty => launch_direct_binary(
            "/Applications/Alacritty.app/Contents/MacOS/alacritty",
            &["-e", "bash", "-c", shell_command],
        ),
        TerminalApp::Kitty => launch_direct_binary(
            "/Applications/kitty.app/Contents/MacOS/kitty",
            &["-e", "bash", "-c", shell_command],
        ),
    }
}

fn launch_terminal_app(cmd: &str) -> anyhow::Result<()> {
    let script = format!(
        "tell application \"Terminal\" to activate\ntell application \"Terminal\" to do script \"{}\"\n",
        escape_applescript(cmd)
    );
    run_osascript(&script)
}

fn launch_iterm2(cmd: &str) -> anyhow::Result<()> {
    let script = format!(
        "tell application \"iTerm\"\nactivate\nif (count of windows) = 0 then\n  create window with default profile\nend if\ntell current window\n  tell current session to write text \"{}\"\nend tell\nend tell\n",
        escape_applescript(cmd)
    );
    run_osascript(&script)
}

/// Warp: URL scheme 안정성이 떨어져 — 대안으로 Warp 앱 활성화 + System Events
/// 로 키스트로크 송신. Accessibility 권한 요구되지만 대부분의 유저가 이미
/// 개발 워크플로우에 권한 부여됨 (keyboard maestro 등).
fn launch_warp(shell_command: &str) -> anyhow::Result<()> {
    let script = format!(
        "tell application \"Warp\" to activate\ndelay 0.35\ntell application \"System Events\"\n  keystroke \"{}\"\n  key code 36\nend tell\n",
        escape_applescript(shell_command)
    );
    run_osascript(&script)
}

fn launch_direct_binary(bin: &str, args: &[&str]) -> anyhow::Result<()> {
    if !std::path::Path::new(bin).exists() {
        anyhow::bail!("{bin} not found — 해당 터미널 앱이 설치됐는지 확인");
    }
    Command::new(bin).args(args).spawn()?;
    Ok(())
}

fn run_osascript(script: &str) -> anyhow::Result<()> {
    let output = Command::new("osascript").args(["-e", script]).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript failed: {stderr}");
    }
    Ok(())
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
