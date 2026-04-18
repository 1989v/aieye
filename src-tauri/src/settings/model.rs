use crate::resume::TerminalApp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub preferred_terminal: TerminalApp,
    pub recent_threshold_minutes: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferred_terminal: TerminalApp::Terminal,
            recent_threshold_minutes: 60,
        }
    }
}
