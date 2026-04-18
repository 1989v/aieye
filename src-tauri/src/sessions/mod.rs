pub mod adapter;
pub mod claude;
pub mod codex;
pub mod coordinator;
pub mod model;

pub use adapter::SessionAdapter;
pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;
pub use coordinator::SessionCoordinator;
pub use model::{CliKind, Session, SessionState};
