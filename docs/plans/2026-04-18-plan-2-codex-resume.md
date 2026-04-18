# aieye Plan 2 — CodexAdapter + Resume + Row Actions

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development.

**Goal:** Plan 1 기반 위에 Codex 세션까지 통합하고, 행 클릭 시 실제 `claude --resume` / `codex resume` 를 사용자 선호 터미널에서 자동 실행. Row action 메뉴 (Reveal in Finder / Copy ID) + Settings 서브메뉴(기본 터미널, recent threshold).

**Architecture:** CodexAdapter 는 `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` 스캔. TerminalLauncher 는 Terminal.app / iTerm2 / Warp / Alacritty / kitty 5개 지원 (muxbar 와 동일 패턴). Settings 는 `@AppStorage` 대신 직접 `tauri-plugin-store` 사용.

**Tech Stack:** Rust (chrono glob), Tauri commands, React context for settings.

---

## File Structure (Plan 2 추가/수정)

```
src-tauri/
├── Cargo.toml                                 # tauri-plugin-shell 추가 (Process 실행)
├── src/
│   ├── lib.rs                                 # Plugin 등록
│   ├── commands.rs                            # resume_session, reveal_in_finder 추가
│   ├── sessions/
│   │   ├── mod.rs                             # CodexAdapter export
│   │   ├── coordinator.rs                     # 신규 — 여러 adapter 병합
│   │   └── codex.rs                           # 신규 — Codex 세션 스캔
│   ├── parser/
│   │   └── codex_jsonl.rs                     # 신규 — Codex rollout JSONL 파서
│   ├── resume/                                # 신규 모듈
│   │   ├── mod.rs
│   │   ├── terminal.rs                        # TerminalApp enum + AppleScript 실행
│   │   └── command.rs                         # 세션 → resume 커맨드 문자열 생성
│   └── settings/
│       ├── mod.rs                             # Settings 로드/저장 (JSON)
│       └── model.rs
└── tests/fixtures/sample-codex.jsonl          # 신규 fixture

src/
├── types/
│   ├── session.ts                             # TerminalApp enum 추가
│   └── settings.ts                            # 신규
├── ipc/
│   └── tauri.ts                               # resume / reveal / settings wrapper 추가
├── hooks/
│   └── useSettings.ts                         # 신규
├── components/
│   ├── SessionRow.tsx                         # onClick + contextMenu 추가 (SessionList 에서 분리)
│   └── SettingsMenu.tsx                       # 신규
└── App.tsx                                    # SettingsMenu 통합
```

---

## Task 1: Codex 파서 + fixture

**Files:**
- Create: `src-tauri/tests/fixtures/sample-codex.jsonl`
- Create: `src-tauri/src/parser/codex_jsonl.rs`
- Modify: `src-tauri/src/parser/mod.rs`

- [ ] **Step 1: fixture**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/tests/fixtures/sample-codex.jsonl`:

```json
{"type":"session_meta","cwd":"/Users/kgd/IdeaProjects/aieye","sessionId":"019cffa9-d1d8-78b0-8c8e-20c5ab8b936f"}
{"type":"user","content":"Write a function to parse yaml"}
{"type":"assistant","content":"Sure, here's a parser..."}
```

(실제 Codex JSONL 포맷은 버전별로 다를 수 있음 — 기본 필드 파악을 위해 위 스키마로 출발, 실제 파일에서 관찰하며 수정)

- [ ] **Step 2: 실제 Codex JSONL 확인**

Run:
```bash
find ~/.codex/sessions -name "rollout-*.jsonl" | head -1 | xargs head -5
```

로그에서 실제 필드 구조 확인. `session_meta` / `user` / `assistant` / `response_item` 등 실제 타입 확인.

- [ ] **Step 3: 파서 작성 (TDD)**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/parser/codex_jsonl.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq)]
pub struct CodexSessionHeader {
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub first_timestamp: Option<DateTime<Utc>>,
}

pub fn read_codex_header(path: &Path) -> anyhow::Result<Option<CodexSessionHeader>> {
    const MAX_LINES: usize = 30;
    const TITLE_LEN: usize = 80;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut cwd: Option<PathBuf> = None;
    let mut first_user: Option<String> = None;
    let mut first_ts: Option<DateTime<Utc>> = None;

    for (i, line) in reader.lines().enumerate() {
        if i >= MAX_LINES {
            break;
        }
        let line = line?;
        if line.is_empty() { continue; }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // cwd from session_meta
        if v.get("type").and_then(|t| t.as_str()) == Some("session_meta") {
            if let Some(c) = v.get("cwd").and_then(|c| c.as_str()) {
                cwd = Some(PathBuf::from(c));
            }
        }

        // timestamp from top-level or nested
        if first_ts.is_none() {
            if let Some(ts) = v.get("timestamp").and_then(|t| t.as_str()) {
                first_ts = DateTime::parse_from_rfc3339(ts).ok().map(|dt| dt.with_timezone(&Utc));
            }
        }

        // first user message
        if first_user.is_none() {
            let role = v.get("role").and_then(|r| r.as_str())
                .or_else(|| v.get("type").and_then(|t| t.as_str()));
            if role == Some("user") {
                if let Some(content) = v.get("content") {
                    first_user = Some(extract_text(content));
                }
            }
        }

        if cwd.is_some() && first_user.is_some() { break; }
    }

    let title = first_user
        .map(|s| truncate(&s, TITLE_LEN))
        .unwrap_or_else(|| "(untitled)".to_string());

    Ok(Some(CodexSessionHeader { title, cwd, first_timestamp: first_ts }))
}

fn extract_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max { return s.to_string(); }
    let t: String = s.chars().take(max).collect();
    format!("{t}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sample-codex.jsonl")
    }

    #[test]
    fn reads_codex_header() {
        let h = read_codex_header(&fixture()).unwrap().unwrap();
        assert_eq!(h.title, "Write a function to parse yaml");
        assert_eq!(h.cwd, Some(std::path::PathBuf::from("/Users/kgd/IdeaProjects/aieye")));
    }
}
```

- [ ] **Step 4: mod.rs export**

Edit `src-tauri/src/parser/mod.rs`:

```rust
pub mod claude_jsonl;
pub mod codex_jsonl;
pub mod project_slug;

pub use claude_jsonl::{read_session_header, SessionHeader};
pub use codex_jsonl::{read_codex_header, CodexSessionHeader};
pub use project_slug::decode_project_slug;
```

- [ ] **Step 5: 테스트 + 커밋**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo test --manifest-path src-tauri/Cargo.toml --lib codex_jsonl
git add -A
git commit -m "feat(parser): Codex rollout JSONL 헤더 파서"
```

---

## Task 2: CodexAdapter

**Files:**
- Create: `src-tauri/src/sessions/codex.rs`
- Modify: `src-tauri/src/sessions/mod.rs`

- [ ] **Step 1: CodexAdapter 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/codex.rs`:

```rust
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

    fn session_from_file(&self, jsonl: &Path) -> Option<Session> {
        let filename = jsonl.file_stem()?.to_string_lossy().to_string();
        // "rollout-2026-03-18T15-37-25-019cffa9-d1d8-78b0-8c8e-20c5ab8b936f"
        // → session id 는 마지막 UUID 부분
        let id = filename
            .rsplit('-')
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("-");

        let metadata = fs::metadata(jsonl).ok()?;
        let mtime: DateTime<Utc> = DateTime::<Utc>::from(metadata.modified().ok()?);

        let header = read_codex_header(jsonl).ok().flatten();
        let title = header.as_ref().map(|h| h.title.clone()).unwrap_or_else(|| "(untitled)".to_string());
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
                    && path.file_name()
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
    fn cli(&self) -> CliKind { CliKind::Codex }

    fn watch_paths(&self) -> Vec<PathBuf> { vec![self.root.clone()] }

    async fn scan(&self) -> anyhow::Result<Vec<Session>> {
        if !self.root.exists() { return Ok(vec![]); }
        let mut sessions = Vec::new();
        self.walk_dir(&self.root, &mut sessions);
        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        Ok(sessions)
    }
}
```

- [ ] **Step 2: mod.rs export**

Edit `src-tauri/src/sessions/mod.rs`:

```rust
pub mod adapter;
pub mod claude;
pub mod codex;
pub mod model;

pub use adapter::SessionAdapter;
pub use claude::ClaudeAdapter;
pub use codex::CodexAdapter;
pub use model::{CliKind, Session, SessionState};
```

- [ ] **Step 3: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add -A
git commit -m "feat(sessions): CodexAdapter — ~/.codex/sessions 재귀 스캔"
```

---

## Task 3: Coordinator — 여러 adapter 병합

**Files:**
- Create: `src-tauri/src/sessions/coordinator.rs`
- Modify: `src-tauri/src/sessions/mod.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Coordinator 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/coordinator.rs`:

```rust
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
```

- [ ] **Step 2: mod.rs export**

Edit `src-tauri/src/sessions/mod.rs`:

```rust
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
```

- [ ] **Step 3: commands.rs 업데이트**

Overwrite `src-tauri/src/commands.rs`:

```rust
use crate::sessions::{Session, SessionCoordinator};

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<Session>, String> {
    let coord = SessionCoordinator::with_defaults();
    Ok(coord.scan_all().await)
}
```

- [ ] **Step 4: Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add -A
git commit -m "feat(sessions): SessionCoordinator — Claude + Codex adapter 병합"
```

---

## Task 4: Resume 커맨드 생성기

**Files:**
- Create: `src-tauri/src/resume/mod.rs`
- Create: `src-tauri/src/resume/command.rs`
- Modify: `src-tauri/src/lib.rs` (mod 등록)

- [ ] **Step 1: command.rs**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/resume/mod.rs`:

```rust
pub mod command;
pub mod terminal;

pub use command::resume_shell_command;
pub use terminal::{launch_in_terminal, TerminalApp};
```

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/resume/command.rs`:

```rust
use crate::sessions::{CliKind, Session};
use std::path::Path;

/// 쉘에서 실행할 완성된 명령어 생성.
/// 예: cd '/Users/kgd/msa' && claude --resume abc-123
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
    use chrono::Utc;
    use std::path::PathBuf;

    fn sample_session(cli: CliKind, id: &str, cwd: Option<&str>) -> Session {
        Session {
            id: id.to_string(),
            cli,
            title: String::new(),
            project_path: cwd.map(PathBuf::from),
            git_branch: None,
            jsonl_path: PathBuf::new(),
            last_activity: Utc::now(),
            message_count: None,
            state: crate::sessions::SessionState::Recent,
        }
    }

    #[test]
    fn claude_with_cwd() {
        let s = sample_session(CliKind::Claude, "abc-123", Some("/Users/kgd/msa"));
        assert_eq!(
            resume_shell_command(&s),
            "cd '/Users/kgd/msa' && claude --resume 'abc-123'"
        );
    }

    #[test]
    fn codex_without_cwd() {
        let s = sample_session(CliKind::Codex, "xyz-789", None);
        assert_eq!(resume_shell_command(&s), "codex resume 'xyz-789'");
    }

    #[test]
    fn quotes_special_chars() {
        let s = sample_session(CliKind::Claude, "id with space", Some("/tmp/a'b"));
        assert_eq!(
            resume_shell_command(&s),
            "cd '/tmp/a'\\''b' && claude --resume 'id with space'"
        );
    }
}
```

- [ ] **Step 2: lib.rs 에 mod 등록**

Edit `src-tauri/src/lib.rs` 최상단에:

```rust
mod commands;
mod parser;
mod resume;
mod sessions;
mod tray;
```

- [ ] **Step 3: 테스트 + 커밋**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib resume
git add -A
git commit -m "feat(resume): resume_shell_command 생성기 + shell quoting"
```

---

## Task 5: Terminal launcher

**Files:**
- Create: `src-tauri/src/resume/terminal.rs`

- [ ] **Step 1: TerminalApp enum + launcher**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/resume/terminal.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalApp {
    Terminal,
    Iterm2,
    Warp,
    Alacritty,
    Kitty,
}

impl TerminalApp {
    pub fn bundle_id(self) -> &'static str {
        match self {
            TerminalApp::Terminal => "com.apple.Terminal",
            TerminalApp::Iterm2 => "com.googlecode.iterm2",
            TerminalApp::Warp => "dev.warp.Warp-Stable",
            TerminalApp::Alacritty => "org.alacritty",
            TerminalApp::Kitty => "net.kovidgoyal.kitty",
        }
    }

    pub fn is_installed(self) -> bool {
        // `mdfind` 로 빠른 확인
        std::process::Command::new("mdfind")
            .args(["-name", "kMDItemCFBundleIdentifier", "==", self.bundle_id()])
            .output()
            .ok()
            .map(|out| !out.stdout.is_empty())
            .unwrap_or(false)
    }
}

pub fn launch_in_terminal(app: TerminalApp, shell_command: &str) -> anyhow::Result<()> {
    match app {
        TerminalApp::Terminal => launch_terminal_app(shell_command),
        TerminalApp::Iterm2 => launch_iterm2(shell_command),
        TerminalApp::Warp | TerminalApp::Alacritty | TerminalApp::Kitty => {
            launch_via_open(app, shell_command)
        }
    }
}

fn launch_terminal_app(cmd: &str) -> anyhow::Result<()> {
    let script = format!(
        r#"tell application "Terminal" to activate
tell application "Terminal" to do script "{}"
"#,
        escape_applescript(cmd)
    );
    run_osascript(&script)
}

fn launch_iterm2(cmd: &str) -> anyhow::Result<()> {
    let script = format!(
        r#"tell application "iTerm"
activate
if (count of windows) = 0 then
  create window with default profile
end if
tell current window
  tell current session to write text "{}"
end tell
end tell
"#,
        escape_applescript(cmd)
    );
    run_osascript(&script)
}

fn launch_via_open(app: TerminalApp, cmd: &str) -> anyhow::Result<()> {
    // open -na <BundleID> --args -e bash -c "<cmd>"
    let status = Command::new("open")
        .args(["-na", "-b", app.bundle_id(), "--args", "-e", "bash", "-c", cmd])
        .status()?;
    if !status.success() {
        anyhow::bail!("open exited with {status:?}");
    }
    Ok(())
}

fn run_osascript(script: &str) -> anyhow::Result<()> {
    let output = Command::new("osascript").args(["-e", script]).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript failed: {stderr}");
    }
    Ok(())
}

fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
```

- [ ] **Step 2: Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add -A
git commit -m "feat(resume): Terminal/iTerm2/Warp/Alacritty/kitty launcher"
```

---

## Task 6: resume_session Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: command 추가**

Edit `src-tauri/src/commands.rs`:

```rust
use crate::resume::{launch_in_terminal, resume_shell_command, TerminalApp};
use crate::sessions::{Session, SessionCoordinator};

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<Session>, String> {
    let coord = SessionCoordinator::with_defaults();
    Ok(coord.scan_all().await)
}

#[tauri::command]
pub async fn resume_session(session: Session, terminal: Option<TerminalApp>) -> Result<(), String> {
    let cmd = resume_shell_command(&session);
    let term = terminal.unwrap_or(TerminalApp::Terminal);
    launch_in_terminal(term, &cmd).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    std::process::Command::new("open")
        .args(["-R", &path])
        .status()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: lib.rs 에 등록**

Edit `src-tauri/src/lib.rs` 의 `invoke_handler`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::list_sessions,
    commands::resume_session,
    commands::reveal_in_finder
])
```

- [ ] **Step 3: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add -A
git commit -m "feat(commands): resume_session + reveal_in_finder tauri commands"
```

---

## Task 7: TypeScript — Resume/Reveal wrapper

**Files:**
- Modify: `src/types/session.ts`
- Modify: `src/ipc/tauri.ts`

- [ ] **Step 1: types/session.ts**

Append to `src/types/session.ts`:

```ts
export type TerminalApp = "terminal" | "iterm2" | "warp" | "alacritty" | "kitty";
```

- [ ] **Step 2: ipc wrapper**

Overwrite `src/ipc/tauri.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Session, TerminalApp } from "../types/session";

export async function listSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_sessions");
}

export async function resumeSession(session: Session, terminal?: TerminalApp): Promise<void> {
  await invoke("resume_session", { session, terminal });
}

export async function revealInFinder(path: string): Promise<void> {
  await invoke("reveal_in_finder", { path });
}
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(ipc): resumeSession + revealInFinder frontend wrappers"
```

---

## Task 8: Row click + ⋯ 메뉴

**Files:**
- Modify: `src/components/SessionList.tsx` → 분리
- Create: `src/components/SessionRow.tsx`

- [ ] **Step 1: SessionRow.tsx 분리 + 클릭·⋯ 메뉴**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/components/SessionRow.tsx`:

```tsx
import { useState } from "react";
import type { Session } from "../types/session";
import { resumeSession, revealInFinder } from "../ipc/tauri";

function relativeTime(iso: string): string {
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

function stateDot(state: Session["state"]): string {
  return state === "running" ? "🟢" : state === "recent" ? "🟡" : "🔘";
}

export function SessionRow({ session }: { session: Session }) {
  const [menuOpen, setMenuOpen] = useState(false);

  const onClick = (e: React.MouseEvent) => {
    // ⋯ 버튼 클릭은 제외
    if ((e.target as HTMLElement).dataset.rowAction) return;
    resumeSession(session).catch((err) => console.error(err));
  };

  return (
    <div className="session-row" onClick={onClick}>
      <span className="state">{stateDot(session.state)}</span>
      <span className="cli">[{session.cli}]</span>
      <div className="body">
        <div className="title">{session.title}</div>
        <div className="sub">
          {session.project_path ?? "unknown path"}
          {session.git_branch && <> · {session.git_branch}</>}
          <> · {relativeTime(session.last_activity)}</>
        </div>
      </div>
      <button
        className="row-menu-btn"
        data-row-action="menu"
        onClick={(e) => {
          e.stopPropagation();
          setMenuOpen((o) => !o);
        }}
      >
        ⋯
      </button>
      {menuOpen && (
        <div className="row-menu" onClick={(e) => e.stopPropagation()}>
          <button
            data-row-action="menu"
            onClick={() => {
              revealInFinder(session.jsonl_path);
              setMenuOpen(false);
            }}
          >
            Reveal in Finder
          </button>
          <button
            data-row-action="menu"
            onClick={() => {
              navigator.clipboard.writeText(session.id);
              setMenuOpen(false);
            }}
          >
            Copy session ID
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: SessionList 리팩토링**

Overwrite `src/components/SessionList.tsx`:

```tsx
import type { Session } from "../types/session";
import { SessionRow } from "./SessionRow";

export function SessionList({ sessions }: { sessions: Session[] }) {
  if (sessions.length === 0) {
    return <div className="empty">No sessions yet.</div>;
  }
  return (
    <div className="session-list">
      {sessions.map((s) => (
        <SessionRow key={`${s.cli}-${s.id}`} session={s} />
      ))}
    </div>
  );
}
```

- [ ] **Step 3: CSS 추가**

Append to `src/styles.css`:

```css
.session-row {
  cursor: pointer;
  position: relative;
}

.session-row:hover {
  background: rgba(255, 255, 255, 0.05);
}

.row-menu-btn {
  background: transparent;
  border: none;
  color: inherit;
  font-size: 14px;
  padding: 4px 6px;
  cursor: pointer;
  opacity: 0.6;
}

.row-menu-btn:hover {
  opacity: 1;
}

.row-menu {
  position: absolute;
  right: 6px;
  top: 32px;
  background: rgba(50, 50, 50, 0.98);
  border-radius: 6px;
  padding: 4px 0;
  min-width: 180px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  z-index: 10;
}

.row-menu button {
  display: block;
  width: 100%;
  background: transparent;
  border: none;
  color: inherit;
  text-align: left;
  padding: 6px 12px;
  font-size: 12px;
  cursor: pointer;
}

.row-menu button:hover {
  background: rgba(255, 255, 255, 0.08);
}
```

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(ui): SessionRow — 클릭 resume + ⋯ 메뉴(Reveal/Copy ID)"
```

---

## Task 9: 전체 빌드 + 수동 테스트

**Files:** 없음 (검증만)

- [ ] **Step 1: 빌드 + 실행**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
pkill -f "aieye.app/Contents/MacOS/aieye" 2>/dev/null
./build.sh open
```

- [ ] **Step 2: 수동 체크리스트**

- [ ] 트레이 클릭 → 패널 뜸
- [ ] Claude 세션들 리스트에 보임
- [ ] Codex 세션들도 리스트에 보임 (있다면)
- [ ] 세션 행 클릭 → Terminal.app 에서 `claude --resume <id>` 실행됨
- [ ] 행 우측 ⋯ 클릭 → 메뉴 뜸
  - [ ] Reveal in Finder → Finder 에서 JSONL 하이라이트
  - [ ] Copy session ID → 클립보드 확인

- [ ] **Step 3: Commit (있다면 fix)**

없으면 skip.

---

## Task 10: Settings 서브메뉴 (preferredTerminal + recentThreshold)

**Files:**
- Create: `src-tauri/src/settings/mod.rs`
- Create: `src-tauri/src/settings/model.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`
- Create: `src/types/settings.ts`
- Create: `src/hooks/useSettings.ts`
- Create: `src/components/SettingsMenu.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Rust Settings 모델**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/settings/model.rs`:

```rust
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
```

- [ ] **Step 2: Settings loader (JSON file)**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/settings/mod.rs`:

```rust
pub mod model;

pub use model::Settings;

use std::path::PathBuf;

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join("Library/Application Support/com.1989v.aieye/settings.json")
}

pub fn load() -> Settings {
    let path = settings_path();
    if !path.exists() {
        return Settings::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, json)?;
    Ok(())
}
```

- [ ] **Step 3: lib.rs 등록 + commands.rs 에 get/set_settings 추가**

Edit `src-tauri/src/lib.rs` 최상단:

```rust
mod commands;
mod parser;
mod resume;
mod sessions;
mod settings;
mod tray;
```

Edit `src-tauri/src/commands.rs` — 추가:

```rust
use crate::settings::{self, Settings};

#[tauri::command]
pub fn get_settings() -> Settings {
    settings::load()
}

#[tauri::command]
pub fn set_settings(settings: Settings) -> Result<(), String> {
    crate::settings::save(&settings).map_err(|e| e.to_string())
}
```

Edit `src-tauri/src/lib.rs` invoke_handler:

```rust
.invoke_handler(tauri::generate_handler![
    commands::list_sessions,
    commands::resume_session,
    commands::reveal_in_finder,
    commands::get_settings,
    commands::set_settings
])
```

Also update `commands::resume_session` to use settings:

Edit `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn resume_session(session: Session) -> Result<(), String> {
    let settings = settings::load();
    let cmd = resume_shell_command(&session);
    launch_in_terminal(settings.preferred_terminal, &cmd).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: TypeScript Settings**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/types/settings.ts`:

```ts
import type { TerminalApp } from "./session";

export interface Settings {
  preferred_terminal: TerminalApp;
  recent_threshold_minutes: number;
}
```

Append to `src/ipc/tauri.ts`:

```ts
import type { Settings } from "../types/settings";

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function setSettings(settings: Settings): Promise<void> {
  await invoke("set_settings", { settings });
}
```

- [ ] **Step 5: useSettings hook**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/hooks/useSettings.ts`:

```ts
import { useCallback, useEffect, useState } from "react";
import { getSettings, setSettings as saveSettings } from "../ipc/tauri";
import type { Settings } from "../types/settings";

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    getSettings().then(setSettings);
  }, []);

  const update = useCallback(async (patch: Partial<Settings>) => {
    setSettings((prev) => {
      const next = { ...(prev ?? { preferred_terminal: "terminal" as const, recent_threshold_minutes: 60 }), ...patch };
      saveSettings(next);
      return next;
    });
  }, []);

  return { settings, update };
}
```

- [ ] **Step 6: SettingsMenu component**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/components/SettingsMenu.tsx`:

```tsx
import { useState } from "react";
import type { TerminalApp } from "../types/session";
import { useSettings } from "../hooks/useSettings";

const TERMINALS: { value: TerminalApp; label: string }[] = [
  { value: "terminal", label: "Terminal" },
  { value: "iterm2", label: "iTerm2" },
  { value: "warp", label: "Warp" },
  { value: "alacritty", label: "Alacritty" },
  { value: "kitty", label: "kitty" },
];

export function SettingsMenu() {
  const { settings, update } = useSettings();
  const [open, setOpen] = useState(false);

  if (!settings) return null;

  return (
    <div className="settings-menu">
      <button
        className="settings-toggle"
        onClick={() => setOpen((o) => !o)}
      >
        ⚙ Settings
      </button>
      {open && (
        <div className="settings-panel">
          <label>
            <span>Preferred terminal</span>
            <select
              value={settings.preferred_terminal}
              onChange={(e) => update({ preferred_terminal: e.target.value as TerminalApp })}
            >
              {TERMINALS.map((t) => (
                <option key={t.value} value={t.value}>{t.label}</option>
              ))}
            </select>
          </label>
          <label>
            <span>Recent threshold (min)</span>
            <input
              type="number"
              min={1}
              max={1440}
              value={settings.recent_threshold_minutes}
              onChange={(e) => update({ recent_threshold_minutes: Number(e.target.value) })}
            />
          </label>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 7: App.tsx 에 추가 + CSS**

Edit `src/App.tsx` — 맨 아래 SessionList 다음에:

```tsx
import { SettingsMenu } from "./components/SettingsMenu";
// ...
return (
  <div className="app">
    {/* header + error + sessions 기존대로 */}
    <SettingsMenu />
  </div>
);
```

Append to `src/styles.css`:

```css
.settings-menu {
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  padding: 4px 0;
}

.settings-toggle {
  width: 100%;
  background: transparent;
  border: none;
  color: inherit;
  padding: 8px 14px;
  text-align: left;
  cursor: pointer;
  font-size: 12px;
}

.settings-toggle:hover {
  background: rgba(255, 255, 255, 0.05);
}

.settings-panel {
  padding: 8px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 11px;
}

.settings-panel label {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
}

.settings-panel select,
.settings-panel input {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: inherit;
  border-radius: 4px;
  padding: 3px 6px;
  font-size: 11px;
  min-width: 100px;
}
```

- [ ] **Step 8: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
./build.sh open
git add -A
git commit -m "feat(settings): 기본 터미널 + recent threshold 저장/로드 (JSON)"
```

---

## Task 11: README + ADR-0002 + 태그

**Files:**
- Modify: `README.md`
- Create: `docs/adr/ADR-0002-adapter-pattern.md`
- Modify: `docs/README.md`

- [ ] **Step 1: ADR-0002**

Create `/Users/gideok-kwon/IdeaProjects/aieye/docs/adr/ADR-0002-adapter-pattern.md`:

```markdown
# ADR-0002: SessionAdapter 트레이트 기반 CLI 확장성

- Status: Accepted
- Date: 2026-04-18

## Context

Claude Code / Codex / Cursor / Aider / Gemini 등 여러 AI CLI 를
통합 리스트로 표시해야 함. 각 CLI 의 세션 저장 경로·JSONL 스키마가
모두 다름.

## Decision

- `SessionAdapter` async trait 정의 (scan / watch_paths / cli)
- CLI 마다 구현: `ClaudeAdapter`, `CodexAdapter`
- `SessionCoordinator` 가 여러 adapter 를 병합해 단일 `Vec<Session>`
  제공
- 새 CLI 추가 = 1 파일 + 1 test 케이스

## Consequences

**장점:**
- 확장 용이. Cursor/Aider 등 추가 시 기존 코드 영향 최소
- 각 adapter 단위 테스트 가능
- Coordinator 에서 중앙 집중 정렬/dedupe

**단점:**
- Box<dyn SessionAdapter> 동적 디스패치 (성능 영향 미미)
- adapter 별 JSONL 포맷 차이 모두 adapter 내부에 숨겨짐 → 새
  CLI 의 unusual 포맷이 나올 때 재설계 필요할 수도
```

- [ ] **Step 2: docs/README.md 업데이트**

Edit `docs/README.md`:

```markdown
# aieye Docs

## Specs
- [v0.1 Design](specs/2026-04-18-v0.1-design.md)

## Plans
- [Plan 1 — Skeleton + ClaudeAdapter](plans/2026-04-18-plan-1-skeleton-claude-adapter.md)
- [Plan 2 — CodexAdapter + Resume](plans/2026-04-18-plan-2-codex-resume.md)
- Plan 3 — Live preview + FS watcher *(예정)*
- Plan 4 — Distribution *(예정)*

## ADRs
- [ADR-0001: Tauri v2 + React 채택](adr/ADR-0001-tauri-v2-react.md)
- [ADR-0002: SessionAdapter 패턴](adr/ADR-0002-adapter-pattern.md)
```

- [ ] **Step 3: README.md status 업데이트**

Edit `README.md` — "> **Status**:" 라인 변경:

```markdown
> **Status**: Plan 2 complete — Claude + Codex sessions, click-to-resume in preferred terminal, row actions, settings persistence.
```

- [ ] **Step 4: Commit + 태그**

```bash
git add -A
git commit -m "docs: Plan 2 완료 — ADR-0002 + README/docs"
git tag -a plan-2-complete -m "Plan 2: Codex + Resume + Settings 완료"
git push origin main --follow-tags
```

---

## Plan 2 완료 기준

- [x] `cargo test --lib` 전 테스트 통과 (Codex 파서, resume command 포함)
- [x] 메뉴바에서 Claude + Codex 세션이 섞여 표시됨
- [x] 세션 클릭 → 설정된 터미널에서 resume 실행
- [x] 행 ⋯ 메뉴 → Reveal in Finder / Copy session ID
- [x] Settings 에서 기본 터미널 + recent threshold 저장, 앱 재기동 후 유지
- [x] `plan-2-complete` 태그 생성
