//! 세션 JSONL 에서 최근 대화 턴 추출. 인라인 1줄 요약(C) + hover 우측 패널(B).
//!
//! 큰 파일 대비: 마지막 256KB 만 읽어 파싱 (세션 규모에 무관한 비용).
//! 단, 첫 몇 줄이 잘려 json 파싱 실패하는 건 무시.

use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const TAIL_BYTES: u64 = 256 * 1024;
const TEXT_MAX: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: TurnRole,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionPreview {
    /// 가장 최근 user 텍스트 (truncate).
    pub last_user: Option<String>,
    /// 가장 최근 assistant 텍스트 (tool_use/thinking 제외, truncate).
    pub last_assistant: Option<String>,
    /// 최신 → 과거 순 정렬된 턴 리스트 (hover 패널용).
    pub recent_turns: Vec<Turn>,
}

pub fn claude_preview(path: &Path) -> SessionPreview {
    let Some(content) = read_tail(path) else { return SessionPreview::default() };
    let mut turns: Vec<Turn> = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ty = v.get("type").and_then(|t| t.as_str());
        let message = match v.get("message") {
            Some(m) => m,
            None => continue,
        };
        let role_str = message.get("role").and_then(|r| r.as_str());
        let ts = v.get("timestamp").and_then(|t| t.as_str()).map(String::from);
        let (role, text) = match (ty, role_str) {
            (Some("user"), Some("user")) => {
                let t = extract_user_text_claude(message.get("content"));
                if t.trim().is_empty() {
                    continue;
                }
                (TurnRole::User, t)
            }
            (Some("assistant"), Some("assistant")) => {
                let t = extract_assistant_text_claude(message.get("content"));
                if t.trim().is_empty() {
                    continue;
                }
                (TurnRole::Assistant, t)
            }
            _ => continue,
        };
        turns.push(Turn {
            role,
            text: truncate(&text, TEXT_MAX),
            timestamp: ts,
        });
    }
    summarize(turns)
}

pub fn codex_preview(path: &Path) -> SessionPreview {
    let Some(content) = read_tail(path) else { return SessionPreview::default() };
    let mut turns: Vec<Turn> = Vec::new();
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
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
        let role = match payload.get("role").and_then(|r| r.as_str()) {
            Some("user") => TurnRole::User,
            Some("assistant") => TurnRole::Assistant,
            _ => continue,
        };
        let text = extract_text_codex(payload.get("content"));
        if text.trim().is_empty() {
            continue;
        }
        let ts = v.get("timestamp").and_then(|t| t.as_str()).map(String::from);
        turns.push(Turn {
            role,
            text: truncate(&text, TEXT_MAX),
            timestamp: ts,
        });
    }
    summarize(turns)
}

fn summarize(mut turns: Vec<Turn>) -> SessionPreview {
    let last_user = turns
        .iter()
        .rev()
        .find(|t| matches!(t.role, TurnRole::User))
        .map(|t| t.text.clone());
    let last_assistant = turns
        .iter()
        .rev()
        .find(|t| matches!(t.role, TurnRole::Assistant))
        .map(|t| t.text.clone());
    // 최신 10턴만 최신순 역정렬
    turns.reverse();
    turns.truncate(10);
    SessionPreview {
        last_user,
        last_assistant,
        recent_turns: turns,
    }
}

fn read_tail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = size.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::with_capacity((size - start) as usize);
    file.read_to_end(&mut buf).ok()?;
    // 중간 라인에서 시작됐다면 첫 라인 버림 (스팸 파싱 에러 감소)
    let s = String::from_utf8_lossy(&buf).to_string();
    if start > 0 {
        if let Some(nl) = s.find('\n') {
            return Some(s[nl + 1..].to_string());
        }
    }
    Some(s)
}

fn extract_user_text_claude(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    item.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn extract_assistant_text_claude(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| {
                // type="text" 만 추림 (tool_use, thinking, tool_result 제외)
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    item.get("text").and_then(|t| t.as_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn extract_text_codex(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max).collect();
    format!("{t}…")
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
            "aieye-preview-test-{}-{}.jsonl",
            std::process::id(),
            n
        ));
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    #[test]
    fn claude_extracts_last_user_assistant() {
        let p = write_tmp(&[
            r#"{"type":"user","message":{"role":"user","content":"hi there"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello back"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"second question"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"x"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x","content":"ok"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"final answer"}]}}"#,
        ]);
        let prev = claude_preview(&p);
        assert_eq!(prev.last_user.as_deref(), Some("second question"));
        assert_eq!(prev.last_assistant.as_deref(), Some("final answer"));
        // 텍스트 없는 턴은 제외되어야 함
        assert_eq!(prev.recent_turns.len(), 4);
        let _ = std::fs::remove_file(&p);
    }
}
