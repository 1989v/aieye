use crate::sessions::{CliKind, Session};
use std::path::Path;

/// 쉘 실행용 resume 명령어 생성. 예:
///   cd '/Users/kgd/msa' && claude --resume 'abc-123'
pub fn resume_shell_command(session: &Session) -> String {
    let cli = match session.cli {
        CliKind::Claude => "claude",
        CliKind::Codex => "codex",
    };
    let resume_arg = match session.cli {
        CliKind::Claude => format!("--resume {}", shell_quote(&session.id)),
        CliKind::Codex => format!("resume {}", shell_quote(&session.id)),
    };
    let prefix = match &session.project_path {
        Some(p) => format!("cd {} && ", shell_quote_path(p)),
        None => String::new(),
    };
    format!("{prefix}{cli} {resume_arg}")
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn shell_quote_path(p: &Path) -> String {
    shell_quote(&p.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::SessionState;
    use chrono::Utc;
    use std::path::PathBuf;

    fn sample(cli: CliKind, id: &str, cwd: Option<&str>) -> Session {
        Session {
            id: id.to_string(),
            cli,
            title: String::new(),
            project_path: cwd.map(PathBuf::from),
            git_branch: None,
            jsonl_path: PathBuf::new(),
            last_activity: Utc::now(),
            message_count: None,
            state: SessionState::Recent,
            running: None,
            finished: false,
            inline_preview: None,
        }
    }

    #[test]
    fn claude_with_cwd() {
        let s = sample(CliKind::Claude, "abc-123", Some("/Users/kgd/msa"));
        assert_eq!(
            resume_shell_command(&s),
            "cd '/Users/kgd/msa' && claude --resume 'abc-123'"
        );
    }

    #[test]
    fn codex_without_cwd() {
        let s = sample(CliKind::Codex, "xyz-789", None);
        assert_eq!(resume_shell_command(&s), "codex resume 'xyz-789'");
    }

    #[test]
    fn quotes_special_chars() {
        let s = sample(CliKind::Claude, "id with space", Some("/tmp/a'b"));
        assert_eq!(
            resume_shell_command(&s),
            "cd '/tmp/a'\\''b' && claude --resume 'id with space'"
        );
    }
}
