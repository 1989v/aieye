use super::model::{CliKind, Session};
use async_trait::async_trait;
use std::path::PathBuf;

#[async_trait]
pub trait SessionAdapter: Send + Sync {
    fn cli(&self) -> CliKind;
    fn watch_paths(&self) -> Vec<PathBuf>;
    async fn scan(&self) -> anyhow::Result<Vec<Session>>;
}
