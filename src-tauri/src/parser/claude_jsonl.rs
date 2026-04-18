use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
pub struct SessionHeader {
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub first_timestamp: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct RawLine<'a> {
    #[serde(rename = "type")]
    line_type: Option<&'a str>,
    message: Option<RawMessage<'a>>,
    cwd: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    timestamp: Option<String>,
}

#[derive(Deserialize)]
struct RawMessage<'a> {
    role: Option<&'a str>,
    content: Option<serde_json::Value>,
}

pub fn read_session_header(path: &Path) -> anyhow::Result<Option<SessionHeader>> {
    const MAX_LINES: usize = 20;
    const TITLE_LEN: usize = 80;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for (i, line) in reader.lines().enumerate() {
        if i >= MAX_LINES {
            break;
        }
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let raw: RawLine = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if raw.line_type != Some("user") {
            continue;
        }
        let message = match raw.message {
            Some(m) => m,
            None => continue,
        };
        if message.role != Some("user") {
            continue;
        }

        let content_text = extract_text(&message.content);
        let title = truncate(&content_text, TITLE_LEN);

        let timestamp = raw
            .timestamp
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        return Ok(Some(SessionHeader {
            title,
            cwd: raw.cwd.map(PathBuf::from),
            git_branch: raw.git_branch,
            first_timestamp: timestamp,
        }));
    }

    Ok(None)
}

fn extract_text(content: &Option<serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-claude.jsonl")
    }

    #[test]
    fn reads_header_from_fixture() {
        let header = read_session_header(&fixture_path())
            .expect("read ok")
            .expect("some header");
        assert_eq!(header.title, "Help me refactor the auth module");
        assert_eq!(
            header.cwd,
            Some(PathBuf::from("/Users/kgd/IdeaProjects/aieye"))
        );
        assert_eq!(header.git_branch, Some("main".to_string()));
        assert!(header.first_timestamp.is_some());
    }

    #[test]
    fn truncates_long_title() {
        let s = "a".repeat(200);
        assert!(truncate(&s, 80).chars().count() <= 81);
    }
}
