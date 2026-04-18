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
    pub fn bundle_id(self) -> &'static str {
        match self {
            TerminalApp::Terminal => "com.apple.Terminal",
            TerminalApp::Iterm2 => "com.googlecode.iterm2",
            TerminalApp::Warp => "dev.warp.Warp-Stable",
            TerminalApp::Alacritty => "org.alacritty",
            TerminalApp::Kitty => "net.kovidgoyal.kitty",
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

/// Warp: URL scheme `warp://action/new_tab?path=<p>&command=<c>`.
/// 값은 URL-encoding.
fn launch_warp(shell_command: &str) -> anyhow::Result<()> {
    // shell_command 는 "cd '<cwd>' && claude --resume '<id>'" 형태.
    // path 는 cwd, command 는 claude 부분만 전달하는 게 Warp UX 에 맞지만,
    // 단순화를 위해 command 한 덩어리 전체를 넘겨 Warp 가 bash 로 실행하게.
    let encoded_cmd = url_encode(shell_command);
    let url = format!("warp://action/new_tab?command={encoded_cmd}");
    let status = Command::new("open").arg(&url).status()?;
    if !status.success() {
        anyhow::bail!("open warp:// exited with {status:?}");
    }
    Ok(())
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

/// RFC3986 기반 간이 URL 인코딩 (unreserved + path-safe 약간).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
