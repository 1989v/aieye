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

/// Claude Code JSONL → 마지막 "의미있는 턴" 으로 활성 판별.
/// - 유저 텍스트 턴이 마지막 → Generating (응답 대기)
/// - assistant tool_use/streaming → Generating (도구 호출 루프 중)
/// - assistant end_turn → Idle
pub fn claude_activity(path: &Path) -> Activity {
    if is_stale(path) {
        return Activity::Idle;
    }
    last_meaningful_activity_claude(path).unwrap_or(Activity::Idle)
}

fn last_meaningful_activity_claude(path: &Path) -> Option<Activity> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last: Option<Activity> = None;
    for line in reader.lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let line_type = v.get("type").and_then(|t| t.as_str());
        let message = match v.get("message") {
            Some(m) => m,
            None => continue,
        };
        let role = message.get("role").and_then(|r| r.as_str());
        let act = match (line_type, role) {
            (Some("user"), Some("user")) => {
                // 진짜 user 텍스트 턴만 카운트 (tool_result 제외)
                if !is_real_user_message(message.get("content")) {
                    continue;
                }
                Activity::Generating
            }
            (Some("assistant"), Some("assistant")) => {
                // stop_reason 으로 완료 여부 판단
                let stop_reason = message.get("stop_reason").and_then(|s| s.as_str());
                match stop_reason {
                    Some("end_turn") | Some("stop_sequence") => Activity::Idle,
                    // "tool_use" / null / "max_tokens" 등은 진행중으로 간주
                    _ => Activity::Generating,
                }
            }
            _ => continue, // system/permission-mode/turn_duration 등
        };
        last = Some(act);
    }
    last
}

fn is_real_user_message(content: Option<&serde_json::Value>) -> bool {
    match content {
        Some(serde_json::Value::String(s)) => !s.trim().is_empty(),
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|item| {
            item.get("type").and_then(|t| t.as_str()) == Some("text")
                && item
                    .get("text")
                    .and_then(|t| t.as_str())
                    .map(|t| !t.trim().is_empty())
                    .unwrap_or(false)
        }),
        _ => false,
    }
}

/// Codex rollout JSONL → 마지막 response_item 의 role.
pub fn codex_activity(path: &Path) -> Activity {
    if is_stale(path) {
        return Activity::Idle;
    }
    last_role_codex(path).unwrap_or(Activity::Idle)
}

fn last_role_codex(path: &Path) -> Option<Activity> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut last: Option<Activity> = None;
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
        let act = match role {
            Some("user") => Activity::Generating,
            Some("assistant") => Activity::Idle,
            _ => continue,
        };
        last = Some(act);
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_tmp(lines: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(
            "aieye-activity-test-{}-{}.jsonl",
            std::process::id(),
            n
        ));
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    #[test]
    fn claude_generating_when_last_is_user() {
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":"hello","stop_reason":"end_turn"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"write code"}}"#,
        ]);
        assert_eq!(claude_activity(&p), Activity::Generating);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn claude_idle_when_assistant_end_turn() {
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":"done","stop_reason":"end_turn"}}"#,
        ]);
        assert_eq!(claude_activity(&p), Activity::Idle);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn claude_generating_when_assistant_tool_use() {
        // 도구 호출 루프 중 — assistant tool_use 가 마지막이라도 진행 중
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"help"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"x"}],"stop_reason":"tool_use"}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","content":"ok"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"y"}],"stop_reason":"tool_use"}}"#,
        ]);
        assert_eq!(claude_activity(&p), Activity::Generating);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn claude_system_meta_ignored() {
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            r#"{"type":"system","subtype":"turn_duration","durationMs":1000}"#,
            r#"{"type":"permission-mode","permissionMode":"bypassPermissions","sessionId":"x"}"#,
        ]);
        assert_eq!(claude_activity(&p), Activity::Generating);
        let _ = std::fs::remove_file(&p);
    }
}
