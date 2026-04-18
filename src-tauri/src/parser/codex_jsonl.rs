use chrono::{DateTime, Utc};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CodexSessionHeader {
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub first_timestamp: Option<DateTime<Utc>>,
}

/// Codex rollout JSONL 의 첫 meaningful user 메시지에서 세션 헤더 추출.
/// - session_meta (payload.cwd) 로 cwd 취득
/// - response_item (payload.role == "user") 중 auto-generated 제외한 첫 항목의 content[].text → title
pub fn read_codex_header(path: &Path) -> anyhow::Result<Option<CodexSessionHeader>> {
    const MAX_LINES: usize = 100;
    const TITLE_LEN: usize = 80;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut cwd: Option<PathBuf> = None;
    let mut first_user: Option<String> = None;
    let mut first_ts: Option<DateTime<Utc>> = None;

    for (i, line) in reader.lines().enumerate() {
        if i >= MAX_LINES {
            break;
        }
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if first_ts.is_none() {
            if let Some(ts) = v.get("timestamp").and_then(|t| t.as_str()) {
                first_ts = DateTime::parse_from_rfc3339(ts)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc));
            }
        }

        let line_type = v.get("type").and_then(|t| t.as_str());
        let payload = v.get("payload");

        // session_meta → cwd
        if cwd.is_none() && line_type == Some("session_meta") {
            if let Some(p) = payload {
                if let Some(c) = p.get("cwd").and_then(|c| c.as_str()) {
                    cwd = Some(PathBuf::from(c));
                }
            }
            continue;
        }

        // response_item with user role → 첫 meaningful 메시지
        if first_user.is_none() && line_type == Some("response_item") {
            if let Some(p) = payload {
                if p.get("type").and_then(|t| t.as_str()) == Some("message")
                    && p.get("role").and_then(|r| r.as_str()) == Some("user")
                {
                    if let Some(content) = p.get("content").and_then(|c| c.as_array()) {
                        let text: String = content
                            .iter()
                            .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join(" ");

                        if !is_auto_generated(&text) {
                            first_user = Some(text);
                        }
                    }
                }
            }
        }

        if cwd.is_some() && first_user.is_some() {
            break;
        }
    }

    let title = first_user
        .map(|s| truncate(&s, TITLE_LEN))
        .unwrap_or_else(|| "(untitled)".to_string());

    Ok(Some(CodexSessionHeader {
        title,
        cwd,
        first_timestamp: first_ts,
    }))
}

/// 시스템/런타임이 자동 주입하는 메타 메시지 필터.
fn is_auto_generated(text: &str) -> bool {
    let prefixes = [
        "<environment_context>",
        "<collaboration_mode>",
        "<skills_instructions>",
        "<permissions instructions>",
        "<memory_instructions>",
        "<user_instructions>",
    ];
    let trimmed = text.trim_start();
    prefixes.iter().any(|p| trimmed.starts_with(p))
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
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-codex.jsonl")
    }

    #[test]
    fn reads_codex_header() {
        let h = read_codex_header(&fixture()).unwrap().unwrap();
        assert_eq!(h.title, "Refactor my yaml parser to handle tabs");
        assert_eq!(h.cwd, Some(PathBuf::from("/Users/kgd/IdeaProjects/aieye")));
        assert!(h.first_timestamp.is_some());
    }

    #[test]
    fn filters_auto_generated_messages() {
        assert!(is_auto_generated("<environment_context>\n  <cwd>/tmp</cwd>"));
        assert!(is_auto_generated("<collaboration_mode>Default</collaboration_mode>"));
        assert!(!is_auto_generated("Refactor this code"));
    }
}
