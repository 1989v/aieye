//! `claude --resume <id> --print "<msg>"` 백그라운드 spawn.
//!
//! Claude only. Codex/running 세션은 commands.rs 가 호출 전에 거름.
//! 30s timeout 후 detach (kill 안 함, 백그라운드 진행).

use crate::sessions::Session;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub async fn send(session: &Session, text: &str) -> Result<(), String> {
    let bin = which_claude().ok_or_else(|| {
        "cli_not_found: claude binary not on PATH. Install Claude Code CLI.".to_string()
    })?;

    let mut child = Command::new(&bin)
        .args(["--resume", &session.id, "--print", text])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("process_failed: spawn claude: {e}"))?;

    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(Ok(status)) if !status.success() => {
            tracing::warn!(
                "claude --resume --print exited non-zero: {:?}",
                status.code()
            );
            Err(format!(
                "process_failed: claude exited with code {:?}",
                status.code()
            ))
        }
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("process_failed: wait: {e}")),
        Err(_) => {
            tracing::info!(
                "headless reply detached (>30s); next poll will pick it up"
            );
            Err("process_timeout: 30s timeout exceeded. Reply continues in background; next poll will pick it up.".to_string())
        }
    }
}

fn which_claude() -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("claude");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
