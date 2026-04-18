use super::adapter::SessionAdapter;
use super::model::{CliKind, Session, SessionState};
use crate::parser::codex_jsonl::read_codex_header;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct CodexAdapter {
    root: PathBuf,
    recent_threshold_minutes: u32,
}

impl CodexAdapter {
    pub fn with_defaults() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Self {
            root: PathBuf::from(home).join(".codex/sessions"),
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

    /// rollout-2026-03-18T15-37-25-019cffa9-d1d8-78b0-8c8e-20c5ab8b936f
    /// → session id 는 마지막 5개 hex 블록 (UUID v7 형태)
    fn extract_session_id(stem: &str) -> String {
        let parts: Vec<&str> = stem.split('-').collect();
        if parts.len() >= 5 {
            let tail = &parts[parts.len() - 5..];
            tail.join("-")
        } else {
            stem.to_string()
        }
    }

    fn session_from_file(&self, jsonl: &Path) -> Option<Session> {
        let stem = jsonl.file_stem()?.to_string_lossy().to_string();
        let id = Self::extract_session_id(&stem);

        let metadata = fs::metadata(jsonl).ok()?;
        let mtime: DateTime<Utc> = DateTime::<Utc>::from(metadata.modified().ok()?);

        let header = read_codex_header(jsonl).ok().flatten();
        let title = header
            .as_ref()
            .map(|h| h.title.clone())
            .unwrap_or_else(|| "(untitled)".to_string());
        let project_path = header.as_ref().and_then(|h| h.cwd.clone());

        Some(Session {
            id,
            cli: CliKind::Codex,
            title,
            project_path,
            git_branch: None,
            jsonl_path: jsonl.to_path_buf(),
            last_activity: mtime,
            message_count: None,
            state: self.classify(mtime),
        })
    }

    fn walk_dir(&self, dir: &Path, out: &mut Vec<Session>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    self.walk_dir(&path, out);
                } else if path.extension().map(|e| e == "jsonl").unwrap_or(false)
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("rollout-"))
                        .unwrap_or(false)
                {
                    if let Some(s) = self.session_from_file(&path) {
                        out.push(s);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl SessionAdapter for CodexAdapter {
    fn cli(&self) -> CliKind {
        CliKind::Codex
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    async fn scan(&self) -> anyhow::Result<Vec<Session>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        let mut sessions = Vec::new();
        self.walk_dir(&self.root, &mut sessions);
        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_session_id_from_rollout_filename() {
        let id = CodexAdapter::extract_session_id(
            "rollout-2026-03-18T15-37-25-019cffa9-d1d8-78b0-8c8e-20c5ab8b936f",
        );
        assert_eq!(id, "019cffa9-d1d8-78b0-8c8e-20c5ab8b936f");
    }
}
