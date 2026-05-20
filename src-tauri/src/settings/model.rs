use crate::resume::TerminalApp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReplyMode {
    Paste,
    Headless,
}

impl Default for ReplyMode {
    fn default() -> Self {
        ReplyMode::Paste
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub preferred_terminal: TerminalApp,
    pub recent_threshold_minutes: u32,
    #[serde(default)]
    pub reply_mode: ReplyMode,
    #[serde(default = "default_group_by_repo")]
    pub group_by_repo: bool,
}

fn default_group_by_repo() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferred_terminal: TerminalApp::Terminal,
            recent_threshold_minutes: 60,
            reply_mode: ReplyMode::Paste,
            group_by_repo: true,
        }
    }
}
