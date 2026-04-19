use super::adapter::SessionAdapter;
use super::model::{CliKind, Session, SessionState};
use crate::parser::{claude_jsonl::read_session_header, project_slug::decode_project_slug};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub struct ClaudeAdapter {
    root: PathBuf,
    recent_threshold_minutes: u32,
}

impl ClaudeAdapter {
    pub fn with_defaults() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Self {
            root: PathBuf::from(home).join(".claude/projects"),
            recent_threshold_minutes: 60,
        }
    }

    pub fn new(root: PathBuf, recent_threshold_minutes: u32) -> Self {
        Self { root, recent_threshold_minutes }
    }

    fn classify(&self, mtime: DateTime<Utc>) -> SessionState {
        let elapsed = Utc::now() - mtime;
        if elapsed.num_minutes() <= self.recent_threshold_minutes as i64 {
            SessionState::Recent
        } else {
            SessionState::Stale
        }
    }

    fn session_from_file(&self, jsonl: &Path, project_slug: &str) -> Option<Session> {
        let id = jsonl.file_stem()?.to_string_lossy().to_string();
        let metadata = fs::metadata(jsonl).ok()?;
        let mtime: DateTime<Utc> = DateTime::<Utc>::from(metadata.modified().ok()?);

        let header = read_session_header(jsonl).ok().flatten();
        let title = header
            .as_ref()
            .map(|h| h.title.clone())
            .unwrap_or_else(|| "(untitled)".to_string());
        let project_path = header
            .as_ref()
            .and_then(|h| h.cwd.clone())
            .or_else(|| decode_project_slug(project_slug));
        let git_branch = header.as_ref().and_then(|h| h.git_branch.clone());

        Some(Session {
            id,
            cli: CliKind::Claude,
            title,
            project_path,
            git_branch,
            jsonl_path: jsonl.to_path_buf(),
            last_activity: mtime,
            message_count: None,
            state: self.classify(mtime),
            running: None,
            finished: false,
            inline_preview: None,
        })
    }
}

#[async_trait]
impl SessionAdapter for ClaudeAdapter {
    fn cli(&self) -> CliKind {
        CliKind::Claude
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    async fn scan(&self) -> anyhow::Result<Vec<Session>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().to_string();
            let dir = entry.path();
            for f in fs::read_dir(&dir)? {
                let f = f?;
                let path = f.path();
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    if let Some(s) = self.session_from_file(&path, &slug) {
                        sessions.push(s);
                    }
                }
            }
        }
        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn scans_fixture_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        let proj_dir = root.join("-Users-kgd-IdeaProjects-aieye");
        fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join("abc-123.jsonl");
        let mut f = File::create(&jsonl).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Test prompt"}},"cwd":"/Users/kgd/IdeaProjects/aieye","sessionId":"abc-123","gitBranch":"main","timestamp":"2026-04-18T12:00:00.000Z"}}"#
        )
        .unwrap();

        let adapter = ClaudeAdapter::new(root.to_path_buf(), 60);
        let sessions = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(adapter.scan())
            .expect("scan ok");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "abc-123");
        assert_eq!(sessions[0].title, "Test prompt");
        assert_eq!(sessions[0].cli, CliKind::Claude);
    }
}
