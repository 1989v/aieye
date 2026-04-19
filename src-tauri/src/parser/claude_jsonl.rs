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
    // 첫 user 메시지가 슬래시 커맨드/시스템 리마인더만 담고 있으면 비게 되므로
    // 유효 제목을 찾을 때까지 최대 60줄까지 스캔.
    const MAX_LINES: usize = 60;
    const TITLE_LEN: usize = 80;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut fallback_cwd: Option<PathBuf> = None;
    let mut fallback_branch: Option<String> = None;
    let mut fallback_ts: Option<DateTime<Utc>> = None;

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

        let timestamp = raw
            .timestamp
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        // 첫 줄에서 cwd/branch/ts 캐시해두고 유효 title 찾을 때까지 재사용
        if fallback_cwd.is_none() {
            fallback_cwd = raw.cwd.clone().map(PathBuf::from);
            fallback_branch = raw.git_branch.clone();
            fallback_ts = timestamp;
        }

        if raw.line_type != Some("user") {
            continue;
        }
        let Some(message) = raw.message else { continue };
        if message.role != Some("user") {
            continue;
        }

        let content_text = extract_text(&message.content);
        let cleaned = strip_command_meta(&content_text);
        if cleaned.is_empty() {
            continue; // 메타만 있는 메시지 — 다음 user 로
        }
        let title = truncate(&cleaned, TITLE_LEN);

        return Ok(Some(SessionHeader {
            title,
            cwd: raw.cwd.map(PathBuf::from).or(fallback_cwd),
            git_branch: raw.git_branch.or(fallback_branch),
            first_timestamp: timestamp.or(fallback_ts),
        }));
    }

    Ok(None)
}

/// Claude Code 가 메시지에 심어넣는 메타 블록을 제거.
/// `<command-name>...</command-name>`, `<command-message>...</command-message>`,
/// `<command-args>...</command-args>`, `<local-command-stdout>...</local-command-stdout>`,
/// `<local-command-caveat>...</local-command-caveat>`, `<system-reminder>...</system-reminder>`,
/// `<user-prompt-submit-hook>...</user-prompt-submit-hook>` 등.
fn strip_command_meta(s: &str) -> String {
    const TAGS: &[&str] = &[
        "command-name",
        "command-message",
        "command-args",
        "local-command-stdout",
        "local-command-stderr",
        "local-command-caveat",
        "system-reminder",
        "user-prompt-submit-hook",
    ];
    let mut out = s.to_string();
    for tag in TAGS {
        out = strip_xml_block(&out, tag);
    }
    out.trim().to_string()
}

fn strip_xml_block(s: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    loop {
        let Some(start) = rest.find(&open) else {
            result.push_str(rest);
            break;
        };
        result.push_str(&rest[..start]);
        let after = &rest[start + open.len()..];
        match after.find(&close) {
            Some(end) => rest = &after[end + close.len()..],
            None => {
                // 닫는 태그 없음 → 해당 위치 이후 전부 버림
                break;
            }
        }
    }
    result
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
