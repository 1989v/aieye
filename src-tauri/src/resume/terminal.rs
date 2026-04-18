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
        TerminalApp::Warp | TerminalApp::Alacritty | TerminalApp::Kitty => {
            launch_via_open(app, shell_command)
        }
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

fn launch_via_open(app: TerminalApp, cmd: &str) -> anyhow::Result<()> {
    let status = Command::new("open")
        .args([
            "-na",
            "-b",
            app.bundle_id(),
            "--args",
            "-e",
            "bash",
            "-c",
            cmd,
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("open exited with {status:?}");
    }
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
