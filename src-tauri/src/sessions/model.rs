use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliKind {
    Claude,
    Codex,
}

impl CliKind {
    pub fn display_name(self) -> &'static str {
        match self {
            CliKind::Claude => "Claude",
            CliKind::Codex => "Codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Running,
    Recent,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub cli: CliKind,
    pub title: String,
    pub project_path: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub jsonl_path: PathBuf,
    pub last_activity: DateTime<Utc>,
    pub message_count: Option<usize>,
    pub state: SessionState,
}
