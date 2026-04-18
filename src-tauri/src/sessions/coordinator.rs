use super::adapter::SessionAdapter;
use super::{ClaudeAdapter, CodexAdapter, Session};

pub struct SessionCoordinator {
    adapters: Vec<Box<dyn SessionAdapter>>,
}

impl SessionCoordinator {
    pub fn with_defaults() -> Self {
        Self {
            adapters: vec![
                Box::new(ClaudeAdapter::with_defaults()),
                Box::new(CodexAdapter::with_defaults()),
            ],
        }
    }

    pub async fn scan_all(&self) -> Vec<Session> {
        let mut all = Vec::new();
        for adapter in &self.adapters {
            match adapter.scan().await {
                Ok(sessions) => all.extend(sessions),
                Err(e) => tracing::warn!("{:?} scan failed: {}", adapter.cli(), e),
            }
        }
        all.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        all
    }
}
