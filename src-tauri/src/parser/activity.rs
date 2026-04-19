//! 세션 JSONL 파일의 마지막 턴으로 활성 상태 판별.
//!
//! heuristic:
//!   - 마지막 메시지가 user role → claude가 응답 생성 중
//!   - 마지막 메시지가 assistant role → 유저 입력 대기
//!   - mtime 이 오래됨 (> IDLE_THRESHOLD) → 무조건 Idle (stale user turn 방지)

use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    time::SystemTime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    /// user 턴이 마지막 — assistant 답변 생성 중으로 추정
    Generating,
    /// assistant 턴이 마지막 — 유저 입력 대기
    Idle,
}

const IDLE_THRESHOLD_SECS: u64 = 300;

/// mtime 이 최근이 아니면 항상 Idle 로 처리.
fn is_stale(path: &Path) -> bool {
    let Ok(md) = std::fs::metadata(path) else {
        return true;
    };
    let Ok(modified) = md.modified() else {
        return true;
    };
    let elapsed = SystemTime::now()
        .duration_since(modified)
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);
    elapsed > IDLE_THRESHOLD_SECS
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Role {
    User,
    Assistant,
}

/// Claude Code JSONL → 마지막 user/assistant role 찾기.
pub fn claude_activity(path: &Path) -> Activity {
    if is_stale(path) {
        return Activity::Idle;
    }
    match last_role_claude(path) {
        Some(Role::User) => Activity::Generating,
        _ => Activity::Idle,
    }
}

fn last_role_claude(path: &Path) -> Option<Role> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last: Option<Role> = None;
    for line in reader.lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let line_type = v.get("type").and_then(|t| t.as_str());
        let role = v
            .get("message")
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str());
        let role = match (line_type, role) {
            (Some("user"), Some("user")) => Role::User,
            (Some("assistant"), Some("assistant")) => Role::Assistant,
            _ => continue,
        };
        last = Some(role);
    }
    last
}

/// Codex rollout JSONL → 마지막 response_item 의 role.
pub fn codex_activity(path: &Path) -> Activity {
    if is_stale(path) {
        return Activity::Idle;
    }
    match last_role_codex(path) {
        Some(Role::User) => Activity::Generating,
        _ => Activity::Idle,
    }
}

fn last_role_codex(path: &Path) -> Option<Role> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last: Option<Role> = None;
    for line in reader.lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("response_item") {
            continue;
        }
        let payload = match v.get("payload") {
            Some(p) => p,
            None => continue,
        };
        if payload.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let role = payload.get("role").and_then(|r| r.as_str());
        let role = match role {
            Some("user") => Role::User,
            Some("assistant") => Role::Assistant,
            _ => continue,
        };
        last = Some(role);
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(lines: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "aieye-activity-test-{}.jsonl",
            std::process::id()
        ));
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    #[test]
    fn claude_generating_when_last_is_user() {
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":"hello"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"write code"}}"#,
        ]);
        assert_eq!(claude_activity(&p), Activity::Generating);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn claude_idle_when_last_is_assistant() {
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":"hello"}}"#,
        ]);
        assert_eq!(claude_activity(&p), Activity::Idle);
        let _ = std::fs::remove_file(&p);
    }
}
