//! AppleScript 기반 paste 전송.
//!
//! running 세션의 host_kind 별로 클립보드 복사 + 활성화 + Cmd+V + Enter 를 수행한다.
//! 클립보드는 best-effort 로 백업/복원. Accessibility 권한 부재 시 osascript 가 실패.

use crate::sessions::Session;
use std::io::Write;
use std::process::Command;
use std::time::Duration;

pub async fn send(session: &Session, text: &str) -> Result<(), String> {
    let running = session
        .running
        .as_ref()
        .ok_or_else(|| "session_not_running: paste requires running session".to_string())?;

    let host_kind = running.host_kind.as_str();

    // iTerm2: native `write text` API. No Accessibility permission needed —
    // only Automation prompt the first time.
    if host_kind == "iterm2" {
        return write_to_iterm(&running.tty, text).await;
    }

    let bundle = match host_kind {
        "terminal" => "Terminal",
        // VS Code / JetBrains / Alacritty / Kitty: host_name 으로 bundle 식별
        "vscode" | "jetbrains" | "other" => {
            running
                .host_name
                .as_deref()
                .ok_or_else(|| format!("host_unsupported: missing host name for {host_kind}"))?
        }
        _ => return Err(format!("host_unsupported: unknown host_kind={host_kind}")),
    };

    let backup = read_clipboard();
    if let Err(e) = write_clipboard(text) {
        return Err(format!("process_failed: clipboard write failed: {e}"));
    }
    if let Err(e) = activate_and_paste(bundle).await {
        if let Some(b) = backup {
            let _ = write_clipboard(&b);
        }
        return Err(e);
    }
    tokio::time::sleep(Duration::from_millis(120)).await;
    if let Some(b) = backup {
        let _ = write_clipboard(&b);
    }
    Ok(())
}

async fn activate_and_paste(bundle_or_name: &str) -> Result<(), String> {
    let escaped = bundle_or_name.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"
tell application "{escaped}" to activate
delay 0.1
tell application "System Events"
    tell process "{escaped}"
        keystroke "v" using command down
        delay 0.05
        keystroke return
    end tell
end tell
"#
    );
    let output = tokio::task::spawn_blocking(move || {
        Command::new("osascript").args(["-e", &script]).output()
    })
    .await
    .map_err(|e| format!("process_failed: spawn join: {e}"))?
    .map_err(|e| format!("process_failed: osascript: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not allowed") || stderr.contains("1002") || stderr.contains("-1719") {
            return Err(format!(
                "permission: Accessibility access required. Enable aieye in System Settings > Privacy > Accessibility. ({stderr})"
            ));
        }
        return Err(format!("process_failed: osascript: {stderr}"));
    }
    Ok(())
}

fn read_clipboard() -> Option<String> {
    let out = Command::new("pbpaste").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn write_clipboard(text: &str) -> Result<(), std::io::Error> {
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    child.wait()?;
    Ok(())
}

/// iTerm2 의 `write text` AppleScript API 로 매칭되는 tty 세션에 직접 쓴다.
/// `keystroke` 를 쓰지 않으므로 Accessibility 권한 불요. iTerm2 자동화 prompt 만 1회 필요.
async fn write_to_iterm(tty: &str, text: &str) -> Result<(), String> {
    let tty_esc = tty.replace('\\', "\\\\").replace('"', "\\\"");
    let text_esc = text.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"
tell application "iTerm"
    activate
    set targetSession to missing value
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                if tty of s is "{tty_esc}" then
                    tell w to select t
                    select s
                    set targetSession to s
                    exit repeat
                end if
            end repeat
            if targetSession is not missing value then exit repeat
        end repeat
        if targetSession is not missing value then exit repeat
    end repeat
    if targetSession is missing value then
        error "session not found for tty {tty_esc}"
    end if
    tell targetSession to write text "{text_esc}"
end tell
"#
    );
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("osascript").args(["-e", &script]).output()
    })
    .await
    .map_err(|e| format!("process_failed: spawn join: {e}"))?
    .map_err(|e| format!("process_failed: osascript: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not authorized") || stderr.contains("not allowed") || stderr.contains("1743") {
            return Err(format!(
                "permission: iTerm2 automation not allowed. Open System Settings > Privacy > Automation > aieye and enable iTerm. ({stderr})"
            ));
        }
        return Err(format!("process_failed: osascript: {stderr}"));
    }
    Ok(())
}
