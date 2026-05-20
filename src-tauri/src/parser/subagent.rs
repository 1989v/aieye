//! Claude Code Task tool 서브에이전트 추출.
//!
//! `tool_use { name: "Task" }` 와 짝이 되는 `tool_result` 를 페어링하여
//! `Vec<SubagentRow>` 를 만든다. tool_result 가 없는 채로 부모 세션이 end_turn
//! 했으면 Errored 로 강등 (orphan).

use crate::sessions::{SubagentRow, SubagentState};
use std::path::Path;

const TEXT_MAX: usize = 120;

/// Claude JSONL 파일에서 Task 서브에이전트들을 시간순(오래된 → 최신) 으로 추출.
pub fn extract_subagents(path: &Path) -> Vec<SubagentRow> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let reader = std::io::BufReader::new(file);

    let mut tasks: Vec<TaskRecord> = Vec::new();
    let mut results: std::collections::HashMap<String, ResultRecord> = std::collections::HashMap::new();
    let mut parent_end_turn = false;

    use std::io::BufRead;
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.is_empty() { continue }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let ty = v.get("type").and_then(|t| t.as_str());
        let ts = v.get("timestamp").and_then(|t| t.as_str()).unwrap_or("").to_string();

        match ty {
            Some("assistant") => {
                let Some(msg) = v.get("message") else { continue };
                if let Some(stop) = msg.get("stop_reason").and_then(|s| s.as_str()) {
                    if stop == "end_turn" {
                        parent_end_turn = true;
                    }
                }
                let Some(content) = msg.get("content").and_then(|c| c.as_array()) else { continue };
                for item in content {
                    let item_ty = item.get("type").and_then(|t| t.as_str());
                    if item_ty != Some("tool_use") { continue }
                    if item.get("name").and_then(|n| n.as_str()) != Some("Task") { continue }
                    let id = item.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                    if id.is_empty() { continue }
                    let input = item.get("input");
                    let subagent_type = input
                        .and_then(|i| i.get("subagent_type"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let description = input
                        .and_then(|i| i.get("description"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    tasks.push(TaskRecord {
                        id,
                        name: subagent_type,
                        description,
                        started_at: ts.clone(),
                    });
                }
            }
            Some("user") => {
                let Some(msg) = v.get("message") else { continue };
                let Some(content) = msg.get("content").and_then(|c| c.as_array()) else { continue };
                for item in content {
                    if item.get("type").and_then(|t| t.as_str()) != Some("tool_result") { continue }
                    let Some(tid) = item.get("tool_use_id").and_then(|i| i.as_str()) else { continue };
                    let is_error = item.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                    let text = extract_result_text(item.get("content"));
                    results.insert(tid.to_string(), ResultRecord {
                        text,
                        is_error,
                        finished_at: ts.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    tasks
        .into_iter()
        .map(|t| {
            let result = results.remove(&t.id);
            let (state, last_text, finished_at) = match result {
                Some(r) if r.is_error => (SubagentState::Errored, Some(truncate(&r.text)), Some(r.finished_at)),
                Some(r) => (SubagentState::Completed, Some(truncate(&r.text)), Some(r.finished_at)),
                None if parent_end_turn => (SubagentState::Errored, None, None),
                None => (SubagentState::Running, None, None),
            };
            SubagentRow {
                id: t.id,
                name: t.name,
                description: t.description,
                state,
                last_text,
                started_at: t.started_at,
                finished_at,
            }
        })
        .collect()
}

struct TaskRecord {
    id: String,
    name: String,
    description: Option<String>,
    started_at: String,
}

struct ResultRecord {
    text: String,
    is_error: bool,
    finished_at: String,
}

fn extract_result_text(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn truncate(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= TEXT_MAX {
        return s.to_string();
    }
    let t: String = s.chars().take(TEXT_MAX).collect();
    format!("{t}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sample-claude-with-subagents.jsonl")
    }

    #[test]
    fn extracts_three_subagents() {
        let rows = extract_subagents(&fixture());
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn first_two_completed_third_errored_orphan() {
        let rows = extract_subagents(&fixture());
        assert_eq!(rows[0].name, "Explore");
        assert_eq!(rows[0].state, SubagentState::Completed);
        assert_eq!(
            rows[0].last_text.as_deref(),
            Some("Found src/auth/login.rs and src/auth/session.rs")
        );
        assert_eq!(rows[1].name, "general-purpose");
        assert_eq!(rows[1].state, SubagentState::Completed);
        assert!(rows[1].last_text.as_deref().unwrap().contains("Refactor plan"));
        assert_eq!(rows[2].name, "general-purpose");
        assert_eq!(rows[2].state, SubagentState::Errored);
        assert_eq!(rows[2].finished_at, None);
    }

    #[test]
    fn description_extracted() {
        let rows = extract_subagents(&fixture());
        assert_eq!(rows[0].description.as_deref(), Some("Find auth-related files"));
    }

    #[test]
    fn missing_file_returns_empty() {
        let rows = extract_subagents(&PathBuf::from("/nonexistent/path.jsonl"));
        assert!(rows.is_empty());
    }
}
