# Session Grouping & In-Panel Reply — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** `docs/specs/2026-05-20-session-grouping-and-send-reply-design.md` (commit `75f4796`)

**Goal:** aieye v0.2 — 세션 리스트를 repo 단위로 그루핑하고, Claude Code Task 서브에이전트를 부모 세션 하위 행으로 표시하고, 패널 안에서 메시지를 직접 전송(Paste/Headless 토글)할 수 있게 만든다.

**Architecture:** Rust 백엔드 (`src-tauri/`)는 Session 모델 확장 + 서브에이전트 JSONL 파서 + AppleScript paste + `claude --resume --print` 헤드리스 spawn IPC를 추가하고, React 프론트엔드(`src/`)는 그루핑 `useMemo` + RepoGroupHeader/SubagentRow + PreviewPane 하단 입력창 + 행 quick-reply 아이콘 + Settings 토글을 추가한다. 기존 `inline_preview` 의 "active 세션만 사전 파싱 + 나머지는 lazy fetch" 패턴을 그대로 재사용.

**Tech Stack:** Tauri v2, Rust (chrono · serde · tokio · anyhow · tracing), React 19 + TypeScript + Vite. macOS only (AppleScript, pbcopy/pbpaste, `claude` 바이너리).

**Constraints:**
- 백엔드/프론트엔드 분리 경계 유지 — JSONL 파싱·프로세스 감지·AppleScript·spawn 은 Rust, 화면 그리기·필터·sort 는 React.
- Settings 신규 필드는 `#[serde(default)]` — 기존 사용자 설정 파일과 forward-compatible.
- FE/BE 의 Session/Settings 타입은 snake_case (기존 컨벤션 유지).

---

## File Structure

### 새 파일

| 경로 | 책임 |
|---|---|
| `src-tauri/src/parser/subagent.rs` | Claude JSONL 에서 Task tool_use/tool_result 페어링하여 `Vec<SubagentRow>` 추출 |
| `src-tauri/src/resume/paste.rs` | host_kind 별 AppleScript paste 디스패치 + 클립보드 백업/복원 |
| `src-tauri/src/resume/headless.rs` | `claude --resume <id> --print "<msg>"` 백그라운드 spawn |
| `src-tauri/tests/fixtures/sample-claude-with-subagents.jsonl` | Task 2개 + orphan 1개 포함 fixture |
| `src/components/RepoGroupHeader.tsx` | 펼침/접힘 헤더 + 메타 |
| `src/components/SubagentRow.tsx` | 들여쓰기된 서브에이전트 자식 행 |

### 수정 파일

| 경로 | 변경 |
|---|---|
| `src-tauri/src/sessions/model.rs` | Session 에 `repo_name`, `subagents` 필드 |
| `src-tauri/src/parser/mod.rs` | `subagent` 모듈 re-export |
| `src-tauri/src/parser/claude_jsonl.rs` | (테스트만 — 새 파서 모듈에 위임) |
| `src-tauri/src/resume/mod.rs` | `paste`, `headless` 모듈 re-export |
| `src-tauri/src/settings/model.rs` | `Settings` 에 `reply_mode`, `group_by_repo` |
| `src-tauri/src/commands.rs` | `list_sessions` 가 running 세션 subagents 사전 채우기, `send_reply` / `get_session_subagents` 신설, `repo_name` 계산 |
| `src-tauri/src/lib.rs` | 신규 IPC handler 등록 |
| `src/types/session.ts` | `repo_name`, `subagents`, `SubagentRow`, `SubagentState` 미러링 |
| `src/types/settings.ts` | `reply_mode`, `group_by_repo`, `ReplyMode` 미러링 |
| `src/ipc/tauri.ts` | `sendReply`, `getSessionSubagents` |
| `src/hooks/useSettings.ts` | 새 필드 기본값 |
| `src/App.tsx` | 그루핑 `useMemo`, `pinned` state, RepoGroup 렌더 |
| `src/components/SessionList.tsx` | 그룹/단일 분기 |
| `src/components/SessionRow.tsx` | 서브에이전트 ul-li 렌더, ✉ 아이콘, pin 호출 |
| `src/components/PreviewPane.tsx` | 하단 reply 입력창 + 상태머신 + pinned 우선 |
| `src/components/SettingsMenu.tsx` | group_by_repo 토글 + reply_mode radio + 권한 안내 |
| `src/styles.css` | 신규 컴포넌트 스타일 |

---

## Task 1: Backend — Session 모델 + Settings 필드

**Files:**
- Modify: `src-tauri/src/sessions/model.rs`
- Modify: `src-tauri/src/settings/model.rs`

- [ ] **Step 1: SubagentRow / SubagentState 추가 (model.rs)**

`src-tauri/src/sessions/model.rs` 끝에 추가:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubagentState {
    Pending,
    Running,
    Completed,
    Errored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentRow {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub state: SubagentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_text: Option<String>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}
```

- [ ] **Step 2: Session 에 repo_name + subagents 필드 추가**

`src-tauri/src/sessions/model.rs:39-57` 의 `Session` 구조체에 마지막 필드로 두 줄 추가:

```rust
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<RunningInfo>,
    #[serde(default)]
    pub finished: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_preview: Option<SessionPreviewInline>,
    /// project_path 의 마지막 세그먼트, None 이면 "(no project)"
    #[serde(default)]
    pub repo_name: String,
    /// Claude Code Task tool 서브에이전트들. lazy fetch 전에는 empty.
    #[serde(default)]
    pub subagents: Vec<SubagentRow>,
}
```

- [ ] **Step 3: sessions/mod.rs 에서 새 타입 re-export**

`src-tauri/src/sessions/mod.rs:11` 의 `pub use model::...` 라인을 다음으로 교체:

```rust
pub use model::{CliKind, Session, SessionPreviewInline, SessionState, SubagentRow, SubagentState};
```

- [ ] **Step 4: Settings 에 reply_mode + group_by_repo 추가**

`src-tauri/src/settings/model.rs` 전체를 다음으로 교체:

```rust
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
```

- [ ] **Step 5: settings/mod.rs 에 ReplyMode re-export**

`src-tauri/src/settings/mod.rs:3` (`pub use model::Settings;`) 를 다음으로 교체:

```rust
pub use model::{ReplyMode, Settings};
```

- [ ] **Step 6: 컴파일 검증**

```bash
cd src-tauri && cargo check
```

Expected: 0 errors, 0 warnings (또는 기존 warning만).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/sessions/model.rs src-tauri/src/sessions/mod.rs \
        src-tauri/src/settings/model.rs src-tauri/src/settings/mod.rs
git commit -m "feat(model): repo_name, subagents, reply_mode, group_by_repo 필드 추가"
```

---

## Task 2: Backend — Sub-agent JSONL 파서

**Files:**
- Create: `src-tauri/src/parser/subagent.rs`
- Create: `src-tauri/tests/fixtures/sample-claude-with-subagents.jsonl`
- Modify: `src-tauri/src/parser/mod.rs`

- [ ] **Step 1: Fixture JSONL 작성**

`src-tauri/tests/fixtures/sample-claude-with-subagents.jsonl` 생성:

```json
{"type":"user","timestamp":"2026-05-20T10:00:00Z","message":{"role":"user","content":"Refactor the auth module"},"cwd":"/Users/kgd/IdeaProjects/aieye","gitBranch":"main"}
{"type":"assistant","timestamp":"2026-05-20T10:00:05Z","message":{"role":"assistant","content":[{"type":"text","text":"I'll explore the codebase first."},{"type":"tool_use","id":"toolu_01","name":"Task","input":{"subagent_type":"Explore","description":"Find auth-related files","prompt":"Search for auth code in src/"}}]}}
{"type":"user","timestamp":"2026-05-20T10:00:30Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_01","content":"Found src/auth/login.rs and src/auth/session.rs","is_error":false}]}}
{"type":"assistant","timestamp":"2026-05-20T10:00:35Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_02","name":"Task","input":{"subagent_type":"general-purpose","description":"Plan the refactor","prompt":"Design new module structure"}}]}}
{"type":"user","timestamp":"2026-05-20T10:01:00Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_02","content":[{"type":"text","text":"Refactor plan: extract SessionStore trait..."}],"is_error":false}]}}
{"type":"assistant","timestamp":"2026-05-20T10:01:30Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_03","name":"Task","input":{"subagent_type":"general-purpose","description":"Run unfinished probe","prompt":"This task will be orphaned"}},{"type":"text","text":"Done.","stop_reason":"end_turn"}],"stop_reason":"end_turn"}}
```

이 fixture 는 3개 Task 호출 포함: 2개 completed, 1개 orphan(tool_result 없음 + stop_reason=end_turn).

- [ ] **Step 2: 실패하는 테스트 먼저 (TDD)**

`src-tauri/src/parser/subagent.rs` 생성 (테스트만 먼저):

```rust
//! Claude Code Task tool 서브에이전트 추출.
//!
//! `tool_use { name: "Task" }` 와 짝이 되는 `tool_result` 를 페어링하여
//! `Vec<SubagentRow>` 를 만든다. tool_result 가 없는 채로 부모 세션이 end_turn
//! 했으면 Errored 로 강등 (orphan).

use crate::sessions::{SubagentRow, SubagentState};
use std::path::Path;

const TEXT_MAX: usize = 120;

/// Claude JSONL 파일에서 Task 서브에이전트들을 시간순(오래된 → 최신) 으로 추출.
pub fn extract_subagents(path: &Path) -> Vec<SubagentRow> {
    // 구현은 Step 3 에서
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sample-claude-with-subagents.jsonl")
    }

    #[test]
    fn extracts_three_subagents() {
        let rows = extract_subagents(&fixture());
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn first_two_completed_third_errored_orphan() {
        let rows = extract_subagents(&fixture());
        assert_eq!(rows[0].name, "Explore");
        assert_eq!(rows[0].state, SubagentState::Completed);
        assert_eq!(
            rows[0].last_text.as_deref(),
            Some("Found src/auth/login.rs and src/auth/session.rs")
        );
        assert_eq!(rows[1].name, "general-purpose");
        assert_eq!(rows[1].state, SubagentState::Completed);
        assert!(rows[1].last_text.as_deref().unwrap().contains("Refactor plan"));
        assert_eq!(rows[2].name, "general-purpose");
        assert_eq!(rows[2].state, SubagentState::Errored);
        assert_eq!(rows[2].finished_at, None);
    }

    #[test]
    fn description_extracted() {
        let rows = extract_subagents(&fixture());
        assert_eq!(rows[0].description.as_deref(), Some("Find auth-related files"));
    }

    #[test]
    fn missing_file_returns_empty() {
        let rows = extract_subagents(&PathBuf::from("/nonexistent/path.jsonl"));
        assert!(rows.is_empty());
    }
}
```

`src-tauri/src/parser/mod.rs` 에 모듈 등록:

```rust
pub mod activity;
pub mod claude_jsonl;
pub mod codex_jsonl;
pub mod preview;
pub mod project_slug;
pub mod subagent;

pub use activity::{claude_activity, codex_activity, Activity};
pub use claude_jsonl::{read_session_header, SessionHeader};
pub use codex_jsonl::{read_codex_header, CodexSessionHeader};
pub use preview::{claude_preview, codex_preview, SessionPreview};
pub use project_slug::decode_project_slug;
pub use subagent::extract_subagents;
```

- [ ] **Step 3: 테스트 실패 확인**

```bash
cd src-tauri && cargo test --lib parser::subagent
```

Expected: 3 tests fail (`extracts_three_subagents`, `first_two_completed_third_errored_orphan`, `description_extracted`), 1 pass (`missing_file_returns_empty`).

- [ ] **Step 4: 파서 구현**

`src-tauri/src/parser/subagent.rs` 의 `extract_subagents` 본문을 다음으로 교체:

```rust
pub fn extract_subagents(path: &Path) -> Vec<SubagentRow> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let reader = std::io::BufReader::new(file);

    // 1차 패스: tool_use Task 모으기 (시간순)
    let mut tasks: Vec<TaskRecord> = Vec::new();
    // 2차 패스에서 tool_use_id → tool_result 매칭
    let mut results: std::collections::HashMap<String, ResultRecord> = std::collections::HashMap::new();
    let mut parent_end_turn = false;

    use std::io::BufRead;
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        if line.is_empty() { continue }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let ty = v.get("type").and_then(|t| t.as_str());
        let ts = v.get("timestamp").and_then(|t| t.as_str()).unwrap_or("").to_string();

        match ty {
            Some("assistant") => {
                let Some(msg) = v.get("message") else { continue };
                if let Some(stop) = msg.get("stop_reason").and_then(|s| s.as_str()) {
                    if stop == "end_turn" {
                        parent_end_turn = true;
                    }
                }
                let Some(content) = msg.get("content").and_then(|c| c.as_array()) else { continue };
                for item in content {
                    let item_ty = item.get("type").and_then(|t| t.as_str());
                    if item_ty != Some("tool_use") { continue }
                    if item.get("name").and_then(|n| n.as_str()) != Some("Task") { continue }
                    let id = item.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                    if id.is_empty() { continue }
                    let input = item.get("input");
                    let subagent_type = input
                        .and_then(|i| i.get("subagent_type"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let description = input
                        .and_then(|i| i.get("description"))
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    tasks.push(TaskRecord {
                        id,
                        name: subagent_type,
                        description,
                        started_at: ts.clone(),
                    });
                }
            }
            Some("user") => {
                let Some(msg) = v.get("message") else { continue };
                let Some(content) = msg.get("content").and_then(|c| c.as_array()) else { continue };
                for item in content {
                    if item.get("type").and_then(|t| t.as_str()) != Some("tool_result") { continue }
                    let Some(tid) = item.get("tool_use_id").and_then(|i| i.as_str()) else { continue };
                    let is_error = item.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                    let text = extract_result_text(item.get("content"));
                    results.insert(tid.to_string(), ResultRecord {
                        text,
                        is_error,
                        finished_at: ts.clone(),
                    });
                }
            }
            _ => {}
        }
    }

    // 페어링 + 상태 결정
    tasks
        .into_iter()
        .map(|t| {
            let result = results.remove(&t.id);
            let (state, last_text, finished_at) = match result {
                Some(r) if r.is_error => (SubagentState::Errored, Some(truncate(&r.text)), Some(r.finished_at)),
                Some(r) => (SubagentState::Completed, Some(truncate(&r.text)), Some(r.finished_at)),
                None if parent_end_turn => (SubagentState::Errored, None, None),
                None => (SubagentState::Running, None, None),
            };
            SubagentRow {
                id: t.id,
                name: t.name,
                description: t.description,
                state,
                last_text,
                started_at: t.started_at,
                finished_at,
            }
        })
        .collect()
}

struct TaskRecord {
    id: String,
    name: String,
    description: Option<String>,
    started_at: String,
}

struct ResultRecord {
    text: String,
    is_error: bool,
    finished_at: String,
}

fn extract_result_text(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(String::from))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn truncate(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= TEXT_MAX {
        return s.to_string();
    }
    let t: String = s.chars().take(TEXT_MAX).collect();
    format!("{t}…")
}
```

- [ ] **Step 5: 테스트 통과 확인**

```bash
cd src-tauri && cargo test --lib parser::subagent
```

Expected: 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/parser/subagent.rs src-tauri/src/parser/mod.rs \
        src-tauri/tests/fixtures/sample-claude-with-subagents.jsonl
git commit -m "feat(parser): Claude Task tool 서브에이전트 추출"
```

---

## Task 3: Backend — repo_name 채우기 + active 세션 subagents

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: list_sessions 에 repo_name 계산 추가**

`src-tauri/src/commands.rs:13-56` 의 `list_sessions` 함수 끝(`Ok(sessions)` 직전)에 다음 블록 추가, 그리고 active subagents 사전 채우기 — `inline_preview` 채우는 루프 직후 `Ok(sessions)` 위:

```rust
    // repo_name: project_path 의 마지막 세그먼트
    for s in sessions.iter_mut() {
        s.repo_name = s
            .project_path
            .as_deref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from)
            .unwrap_or_else(|| "(no project)".to_string());
    }

    // active(running) 세션의 서브에이전트를 첫 응답에 동봉 (최대 20)
    let active_count = sessions.iter().filter(|s| s.running.is_some()).count().min(20);
    let mut filled = 0usize;
    for s in sessions.iter_mut() {
        if filled >= active_count { break }
        if s.running.is_none() { continue }
        if !matches!(s.cli, CliKind::Claude) { continue }
        s.subagents = crate::parser::extract_subagents(&s.jsonl_path);
        filled += 1;
    }

    Ok(sessions)
```

(기존 마지막 `Ok(sessions)` 라인을 새 블록 끝의 것이 대체하므로 중복 제거.)

- [ ] **Step 2: get_session_subagents IPC 추가**

`src-tauri/src/commands.rs` 의 `archive_sessions_bulk` 함수 직후에 추가:

```rust
#[tauri::command]
pub fn get_session_subagents(
    jsonl_path: String,
    cli: CliKind,
) -> Vec<crate::sessions::SubagentRow> {
    if !matches!(cli, CliKind::Claude) {
        return Vec::new();
    }
    crate::parser::extract_subagents(&PathBuf::from(jsonl_path))
}
```

- [ ] **Step 3: lib.rs 에 핸들러 등록**

`src-tauri/src/lib.rs:23-35` 의 `invoke_handler` 호출에 `commands::get_session_subagents` 추가. 기존 라인:

```rust
            commands::archive_sessions_bulk
```

다음으로 교체:

```rust
            commands::archive_sessions_bulk,
            commands::get_session_subagents
```

- [ ] **Step 4: 컴파일 검증**

```bash
cd src-tauri && cargo check
```

Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(ipc): repo_name 계산 + active 세션 subagents 사전 채우기 + get_session_subagents"
```

---

## Task 4: Backend — Paste 전송 (AppleScript)

**Files:**
- Create: `src-tauri/src/resume/paste.rs`
- Modify: `src-tauri/src/resume/mod.rs`

- [ ] **Step 1: paste.rs 작성**

`src-tauri/src/resume/paste.rs` 생성:

```rust
//! AppleScript 기반 paste 전송.
//!
//! running 세션의 host_kind 별로 클립보드 복사 + 활성화 + Cmd+V + Enter 를 수행한다.
//! 클립보드는 best-effort 로 백업/복원. Accessibility 권한 부재 시 osascript 가 실패.

use crate::sessions::Session;
use std::io::Write;
use std::process::Command;
use std::time::Duration;

pub async fn send(session: &Session, text: &str) -> Result<(), String> {
    let running = session
        .running
        .as_ref()
        .ok_or_else(|| "session_not_running: paste requires running session".to_string())?;

    let host_kind = running.host_kind.as_str();
    let bundle = match host_kind {
        "terminal" => "Terminal",
        "iterm2" => "iTerm",
        // Alacritty/Kitty: host_kind 는 "other" 로 들어옴. 이름이 있으면 사용.
        "other" => running.host_name.as_deref().unwrap_or("Terminal"),
        "vscode" | "jetbrains" => {
            return Err(format!(
                "host_unsupported: paste 는 Terminal/iTerm2 에서만 지원합니다 (현재: {host_kind})"
            ));
        }
        _ => return Err(format!("host_unsupported: unknown host_kind={host_kind}")),
    };

    let backup = read_clipboard();
    if let Err(e) = write_clipboard(text) {
        return Err(format!("process_failed: clipboard write failed: {e}"));
    }
    if let Err(e) = activate_and_paste(bundle).await {
        // 복원만 시도하고 에러는 그대로 반환
        if let Some(b) = backup {
            let _ = write_clipboard(&b);
        }
        return Err(e);
    }
    tokio::time::sleep(Duration::from_millis(120)).await;
    if let Some(b) = backup {
        let _ = write_clipboard(&b);
    }
    Ok(())
}

async fn activate_and_paste(bundle_or_name: &str) -> Result<(), String> {
    let escaped = bundle_or_name.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"
tell application "{escaped}" to activate
delay 0.1
tell application "System Events"
    tell process "{escaped}"
        keystroke "v" using command down
        delay 0.05
        keystroke return
    end tell
end tell
"#
    );
    let output = tokio::task::spawn_blocking(move || {
        Command::new("osascript").args(["-e", &script]).output()
    })
    .await
    .map_err(|e| format!("process_failed: spawn join: {e}"))?
    .map_err(|e| format!("process_failed: osascript: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("not allowed") || stderr.contains("1002") || stderr.contains("-1719") {
            return Err(format!(
                "permission: Accessibility access required. Enable aieye in System Settings > Privacy > Accessibility. ({stderr})"
            ));
        }
        return Err(format!("process_failed: osascript: {stderr}"));
    }
    Ok(())
}

fn read_clipboard() -> Option<String> {
    let out = Command::new("pbpaste").output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn write_clipboard(text: &str) -> Result<(), std::io::Error> {
    let mut child = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    child.wait()?;
    Ok(())
}
```

- [ ] **Step 2: resume/mod.rs 에 등록**

`src-tauri/src/resume/mod.rs` 전체를 다음으로 교체:

```rust
pub mod command;
pub mod headless;
pub mod paste;
pub mod running;
pub mod terminal;

pub use command::resume_shell_command;
pub use running::{
    find_running, match_running, snapshot_running, HostApp, RunningInfo, RunningSession,
};
pub use terminal::{
    focus_existing_tab, launch_in_terminal, activate_app, TerminalApp,
};
```

(`headless` 는 다음 Task 에서 추가, 미리 선언만 — 다음 Task 의 mod.rs 변경은 생략 가능.)

`headless` 가 아직 없으므로 이 Task 에선 `pub mod headless;` 라인을 빼고 paste 만 추가:

```rust
pub mod command;
pub mod paste;
pub mod running;
pub mod terminal;

pub use command::resume_shell_command;
pub use running::{
    find_running, match_running, snapshot_running, HostApp, RunningInfo, RunningSession,
};
pub use terminal::{
    focus_existing_tab, launch_in_terminal, activate_app, TerminalApp,
};
```

- [ ] **Step 3: 컴파일 검증**

```bash
cd src-tauri && cargo check
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/resume/paste.rs src-tauri/src/resume/mod.rs
git commit -m "feat(resume): AppleScript 기반 paste 전송 모듈"
```

---

## Task 5: Backend — Headless 전송 (`claude --resume --print`)

**Files:**
- Create: `src-tauri/src/resume/headless.rs`
- Modify: `src-tauri/src/resume/mod.rs`

- [ ] **Step 1: headless.rs 작성**

`src-tauri/src/resume/headless.rs` 생성:

```rust
//! `claude --resume <id> --print "<msg>"` 백그라운드 spawn.
//!
//! Claude only. Codex/running 세션은 commands.rs 가 호출 전에 거름.
//! 30s timeout 후 detach (kill 안 함, 백그라운드 진행).

use crate::sessions::Session;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub async fn send(session: &Session, text: &str) -> Result<(), String> {
    let bin = which_claude().ok_or_else(|| {
        "cli_not_found: claude binary not on PATH. Install Claude Code CLI.".to_string()
    })?;

    let mut child = Command::new(&bin)
        .args(["--resume", &session.id, "--print", text])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("process_failed: spawn claude: {e}"))?;

    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(Ok(status)) if !status.success() => {
            tracing::warn!(
                "claude --resume --print exited non-zero: {:?}",
                status.code()
            );
            Err(format!(
                "process_failed: claude exited with code {:?}",
                status.code()
            ))
        }
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("process_failed: wait: {e}")),
        Err(_) => {
            tracing::info!(
                "headless reply detached (>30s); next poll will pick it up"
            );
            // detach: child drop 시 kill 안 함 (tokio Command 기본). 일단 OK 반환.
            Err("process_timeout: 응답 대기 30초 초과. 백그라운드에서 진행 중입니다.".to_string())
        }
    }
}

fn which_claude() -> Option<std::path::PathBuf> {
    // which 크레이트 안 쓰고 PATH 직접 스캔 (의존성 절감)
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("claude");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
```

- [ ] **Step 2: resume/mod.rs 에 headless 추가**

`src-tauri/src/resume/mod.rs` 의 `pub mod` 블록을 다음으로 교체:

```rust
pub mod command;
pub mod headless;
pub mod paste;
pub mod running;
pub mod terminal;
```

- [ ] **Step 3: 컴파일 검증**

```bash
cd src-tauri && cargo check
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/resume/headless.rs src-tauri/src/resume/mod.rs
git commit -m "feat(resume): claude --resume --print 헤드리스 spawn"
```

---

## Task 6: Backend — send_reply IPC + dispatch

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: send_reply 명령 추가**

`src-tauri/src/commands.rs` 의 `set_settings` 함수 직후에 추가:

```rust
#[tauri::command]
pub async fn send_reply(session: Session, text: String) -> Result<(), String> {
    use crate::settings::ReplyMode;

    if text.trim().is_empty() {
        return Err("process_failed: empty message".into());
    }

    let cfg = crate::settings::load();
    let mode = cfg.reply_mode;

    // Headless 모드 가드: Claude only + idle only
    if matches!(mode, ReplyMode::Headless) {
        if !matches!(session.cli, CliKind::Claude) {
            return Err(
                "host_unsupported: Headless 모드는 Claude 세션만 지원합니다.".into()
            );
        }
        let generating = session
            .running
            .as_ref()
            .and_then(|r| r.activity.as_ref())
            .map(|a| matches!(a, crate::parser::Activity::Generating))
            .unwrap_or(false);
        if generating {
            return Err(
                "session_running: 세션이 응답 중입니다. paste 모드로 폴백하거나 잠시 후 시도하세요.".into()
            );
        }
        return crate::resume::headless::send(&session, &text).await;
    }

    // Paste 모드
    crate::resume::paste::send(&session, &text).await
}
```

- [ ] **Step 2: lib.rs 에 send_reply 핸들러 등록**

`src-tauri/src/lib.rs` 의 `invoke_handler` 마지막 라인 (`commands::get_session_subagents`) 다음에 추가:

```rust
            commands::get_session_subagents,
            commands::send_reply
```

- [ ] **Step 3: 컴파일 검증**

```bash
cd src-tauri && cargo check
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(ipc): send_reply 명령 + paste/headless 디스패치"
```

---

## Task 7: Frontend — 타입 미러링 + IPC 래퍼

**Files:**
- Modify: `src/types/session.ts`
- Modify: `src/types/settings.ts`
- Modify: `src/ipc/tauri.ts`
- Modify: `src/hooks/useSettings.ts`

- [ ] **Step 1: Session 타입 확장**

`src/types/session.ts` 전체를 다음으로 교체:

```typescript
export type CliKind = "claude" | "codex";

export type SessionState = "running" | "recent" | "stale";

export type TerminalApp = "terminal" | "iterm2" | "alacritty" | "kitty";

export type HostKind = "terminal" | "iterm2" | "vscode" | "jetbrains" | "other";

export type Activity = "generating" | "idle";

export interface RunningInfo {
  pid: number;
  tty: string;
  host_kind: HostKind;
  host_name: string | null;
  activity?: Activity | null;
}

export interface SessionPreviewInline {
  last_user?: string | null;
  last_assistant?: string | null;
}

export type TurnRole = "user" | "assistant";

export interface Turn {
  role: TurnRole;
  text: string;
  timestamp?: string | null;
}

export interface SessionPreview {
  last_user?: string | null;
  last_assistant?: string | null;
  recent_turns: Turn[];
}

export type SubagentState = "pending" | "running" | "completed" | "errored";

export interface SubagentRow {
  id: string;
  name: string;
  description?: string | null;
  state: SubagentState;
  last_text?: string | null;
  started_at: string;
  finished_at?: string | null;
}

export interface Session {
  id: string;
  cli: CliKind;
  title: string;
  project_path: string | null;
  git_branch: string | null;
  jsonl_path: string;
  last_activity: string;
  message_count: number | null;
  state: SessionState;
  running?: RunningInfo | null;
  finished?: boolean;
  inline_preview?: SessionPreviewInline | null;
  repo_name: string;
  subagents: SubagentRow[];
}
```

- [ ] **Step 2: Settings 타입 확장**

`src/types/settings.ts` 전체를 다음으로 교체:

```typescript
import type { TerminalApp } from "./session";

export type ReplyMode = "paste" | "headless";

export interface Settings {
  preferred_terminal: TerminalApp;
  recent_threshold_minutes: number;
  reply_mode: ReplyMode;
  group_by_repo: boolean;
}
```

- [ ] **Step 3: ipc/tauri.ts 에 새 명령 추가**

`src/ipc/tauri.ts` 끝에 추가:

```typescript
import type { SubagentRow } from "../types/session";

export async function sendReply(session: Session, text: string): Promise<void> {
  await invoke("send_reply", { session, text });
}

export async function getSessionSubagents(
  jsonlPath: string,
  cli: CliKind,
): Promise<SubagentRow[]> {
  return invoke<SubagentRow[]>("get_session_subagents", { jsonlPath, cli });
}
```

(파일 상단의 `import type { ... } from "../types/session";` 라인에 `SubagentRow` 도 함께 넣을 수도 있음 — 둘 중 하나만 유지.)

- [ ] **Step 4: useSettings 기본값 업데이트**

`src/hooks/useSettings.ts:14` 의 `base` fallback 을 다음으로 교체:

```typescript
      const base: Settings = prev ?? {
        preferred_terminal: "terminal",
        recent_threshold_minutes: 60,
        reply_mode: "paste",
        group_by_repo: true,
      };
```

- [ ] **Step 5: 타입 체크**

```bash
pnpm tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add src/types/session.ts src/types/settings.ts src/ipc/tauri.ts src/hooks/useSettings.ts
git commit -m "feat(types): SubagentRow, ReplyMode, group_by_repo 미러링 + sendReply IPC"
```

---

## Task 8: Frontend — 그루핑 + RepoGroupHeader

**Files:**
- Create: `src/components/RepoGroupHeader.tsx`
- Modify: `src/App.tsx`
- Modify: `src/components/SessionList.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: RepoGroupHeader 작성**

`src/components/RepoGroupHeader.tsx` 생성:

```tsx
interface Props {
  repoName: string;
  sessionCount: number;
  latestRelative: string;
  collapsed: boolean;
  onToggle: () => void;
}

export function RepoGroupHeader({
  repoName,
  sessionCount,
  latestRelative,
  collapsed,
  onToggle,
}: Props) {
  return (
    <button className="repo-group-header" onClick={onToggle} aria-expanded={!collapsed}>
      <span className={`caret ${collapsed ? "right" : "down"}`}>{collapsed ? "▸" : "▾"}</span>
      <span className="repo-name">{repoName}</span>
      <span className="repo-count">{sessionCount}</span>
      <span className="repo-latest">{latestRelative}</span>
    </button>
  );
}
```

- [ ] **Step 2: App.tsx 에 그루핑 useMemo + collapsed state**

`src/App.tsx` 의 `import` 블록과 컴포넌트 본문에 다음 추가/수정:

상단 import:

```typescript
import { useSettings } from "./hooks/useSettings";
import { RepoGroupHeader } from "./components/RepoGroupHeader";
```

`useSettings` 훅 호출 (기존 `useSessions` 다음):

```typescript
  const { settings } = useSettings();
```

`filtered` `useMemo` 직후에 다음 블록 추가:

```typescript
  interface RepoGroup {
    repoName: string;
    sessions: Session[];
    latestActivity: string;
  }

  const grouped = useMemo<RepoGroup[]>(() => {
    if (!settings?.group_by_repo) {
      return [{ repoName: "", sessions: filtered, latestActivity: "" }];
    }
    const map = new Map<string, Session[]>();
    for (const s of filtered) {
      const key = s.repo_name || "(no project)";
      const arr = map.get(key);
      if (arr) arr.push(s);
      else map.set(key, [s]);
    }
    const groups: RepoGroup[] = [];
    for (const [name, list] of map) {
      list.sort((a, b) => b.last_activity.localeCompare(a.last_activity));
      groups.push({ repoName: name, sessions: list, latestActivity: list[0].last_activity });
    }
    groups.sort((a, b) => {
      if (a.repoName === "(no project)") return 1;
      if (b.repoName === "(no project)") return -1;
      return b.latestActivity.localeCompare(a.latestActivity);
    });
    return groups;
  }, [filtered, settings?.group_by_repo]);

  const [collapsedRepos, setCollapsedRepos] = useState<Set<string>>(new Set());
  const toggleRepo = (name: string) => {
    setCollapsedRepos((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };
```

`<SessionList sessions={filtered} ... />` 호출을 다음으로 교체:

```tsx
        {sessions && (
          <SessionList
            groups={grouped}
            collapsedRepos={collapsedRepos}
            onToggleRepo={toggleRepo}
            groupByRepo={settings?.group_by_repo ?? true}
            onHover={setHovered}
            manageMode={manageMode}
            selected={selected}
            eligibleIds={eligibleIds}
            onToggleSelect={toggleSelect}
          />
        )}
```

- [ ] **Step 3: SessionList.tsx 업데이트**

`src/components/SessionList.tsx` 전체를 다음으로 교체:

```tsx
import type { Session } from "../types/session";
import { SessionRow } from "./SessionRow";
import { RepoGroupHeader } from "./RepoGroupHeader";

interface RepoGroup {
  repoName: string;
  sessions: Session[];
  latestActivity: string;
}

interface Props {
  groups: RepoGroup[];
  collapsedRepos: Set<string>;
  onToggleRepo: (name: string) => void;
  groupByRepo: boolean;
  onHover?: (session: Session | null) => void;
  manageMode?: boolean;
  selected?: Set<string>;
  eligibleIds?: Set<string>;
  onToggleSelect?: (id: string) => void;
}

function relativeTime(iso: string): string {
  if (!iso) return "";
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

export function SessionList({
  groups,
  collapsedRepos,
  onToggleRepo,
  groupByRepo,
  onHover,
  manageMode,
  selected,
  eligibleIds,
  onToggleSelect,
}: Props) {
  const totalSessions = groups.reduce((sum, g) => sum + g.sessions.length, 0);
  if (totalSessions === 0) {
    return <div className="empty">No sessions yet.</div>;
  }
  return (
    <div className="session-list" onMouseLeave={() => onHover?.(null)}>
      {groups.map((g) => {
        const isCollapsed = collapsedRepos.has(g.repoName);
        const showHeader = groupByRepo && (groups.length > 1 || g.repoName !== "");
        return (
          <div key={g.repoName || "_flat"} className="repo-group">
            {showHeader && (
              <RepoGroupHeader
                repoName={g.repoName}
                sessionCount={g.sessions.length}
                latestRelative={relativeTime(g.latestActivity)}
                collapsed={isCollapsed}
                onToggle={() => onToggleRepo(g.repoName)}
              />
            )}
            {!isCollapsed &&
              g.sessions.map((s) => (
                <SessionRow
                  key={`${s.cli}-${s.id}`}
                  session={s}
                  onHover={onHover}
                  manageMode={manageMode}
                  selected={selected?.has(s.id)}
                  eligible={eligibleIds?.has(s.id) ?? false}
                  onToggleSelect={onToggleSelect}
                />
              ))}
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 4: 그룹 헤더 스타일**

`src/styles.css` 끝에 추가:

```css
.repo-group {
  display: flex;
  flex-direction: column;
}
.repo-group-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.03);
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  color: var(--text-muted, #9aa);
  cursor: pointer;
  font-size: 11px;
  text-align: left;
}
.repo-group-header:hover {
  background: rgba(255, 255, 255, 0.06);
}
.repo-group-header .caret {
  width: 10px;
  font-size: 10px;
}
.repo-group-header .repo-name {
  font-weight: 600;
  color: var(--text, #ddd);
}
.repo-group-header .repo-count {
  background: rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  padding: 1px 6px;
  font-size: 10px;
}
.repo-group-header .repo-latest {
  margin-left: auto;
  font-size: 10px;
  opacity: 0.6;
}
```

- [ ] **Step 5: 타입 체크**

```bash
pnpm tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add src/components/RepoGroupHeader.tsx src/components/SessionList.tsx \
        src/App.tsx src/styles.css
git commit -m "feat(panel): 세션 리스트 repo 단위 그루핑 + 펼침/접힘"
```

---

## Task 9: Frontend — Subagent 행 + lazy fetch

**Files:**
- Create: `src/components/SubagentRow.tsx`
- Modify: `src/components/SessionRow.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: SubagentRow 작성**

`src/components/SubagentRow.tsx` 생성:

```tsx
import type { SubagentRow as SubagentRowData } from "../types/session";

function stateDot(state: SubagentRowData["state"]): string {
  switch (state) {
    case "running": return "●";
    case "completed": return "✓";
    case "errored": return "✕";
    default: return "○";
  }
}

function relativeTime(iso?: string | null): string {
  if (!iso) return "";
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

interface Props {
  row: SubagentRowData;
}

export function SubagentRow({ row }: Props) {
  const time = row.finished_at ?? row.started_at;
  return (
    <div className={`subagent-row state-${row.state}`}>
      <span className="subagent-glyph">↳</span>
      <span className="subagent-dot" aria-label={row.state}>
        {stateDot(row.state)}
      </span>
      <span className="subagent-name">{row.name}</span>
      {row.description && <span className="subagent-desc">{row.description}</span>}
      <span className="subagent-time">{relativeTime(time)}</span>
    </div>
  );
}
```

- [ ] **Step 2: SessionRow 에 subagents 렌더 + lazy fetch**

`src/components/SessionRow.tsx` 의 imports 와 컴포넌트 본문 수정:

상단 import 추가:

```typescript
import { archiveSessionFile, getSessionSubagents, resumeSession, resumeSessionForceNew, revealInFinder } from "../ipc/tauri";
import { SubagentRow } from "./SubagentRow";
import type { Session, SubagentRow as SubagentRowData } from "../types/session";
```

(기존 `import type { Session } from "../types/session";` 라인 삭제 — 새 import 가 대체.)

컴포넌트 본문 내부, `const [confirmArchive, setConfirmArchive] = useState(false);` 다음 줄에 추가:

```typescript
  const [subagents, setSubagents] = useState<SubagentRowData[]>(session.subagents ?? []);
  const [subagentsFetched, setSubagentsFetched] = useState(
    (session.subagents ?? []).length > 0,
  );

  useEffect(() => {
    // 세션 객체가 갱신되면 사전 채워진 subagents 동기화
    if (session.subagents && session.subagents.length > 0) {
      setSubagents(session.subagents);
      setSubagentsFetched(true);
    }
  }, [session.subagents]);

  useEffect(() => {
    // Claude 세션 + 사전 채움 없음 → lazy fetch (1회)
    if (session.cli !== "claude") return;
    if (subagentsFetched) return;
    let cancelled = false;
    getSessionSubagents(session.jsonl_path, session.cli)
      .then((rows) => {
        if (!cancelled) {
          setSubagents(rows);
          setSubagentsFetched(true);
        }
      })
      .catch((e) => console.error("getSessionSubagents failed", e));
    return () => {
      cancelled = true;
    };
  }, [session.jsonl_path, session.cli, subagentsFetched]);
```

return JSX 의 root `<div>` 를 fragment 로 감싸지 말고, 기존 `</div>` 직전(`<ConfirmDialog ...>` 직전)에 다음 추가:

```tsx
      {subagents.length > 0 && (
        <ul className="subagent-list">
          {subagents.map((sa) => (
            <li key={sa.id}>
              <SubagentRow row={sa} />
            </li>
          ))}
        </ul>
      )}
```

- [ ] **Step 3: 스타일 추가**

`src/styles.css` 끝에 추가:

```css
.subagent-list {
  list-style: none;
  padding: 0;
  margin: 4px 0 0 0;
}
.subagent-list li {
  padding: 0;
}
.subagent-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 12px 3px 36px;
  font-size: 11px;
  color: var(--text-muted, #9aa);
  border-left: 2px solid rgba(255, 255, 255, 0.06);
  margin-left: 24px;
}
.subagent-row .subagent-glyph {
  opacity: 0.5;
}
.subagent-row .subagent-dot {
  width: 10px;
  text-align: center;
  font-size: 10px;
}
.subagent-row.state-running .subagent-dot { color: #6cb6ff; animation: blink 1.4s infinite; }
.subagent-row.state-completed .subagent-dot { color: #6cd07a; }
.subagent-row.state-errored .subagent-dot { color: #e57373; }
.subagent-row.state-pending .subagent-dot { color: #999; }
.subagent-row .subagent-name {
  color: var(--text, #ddd);
  font-weight: 500;
}
.subagent-row .subagent-desc {
  opacity: 0.7;
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.subagent-row .subagent-time {
  font-size: 10px;
  opacity: 0.5;
}
@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
```

- [ ] **Step 4: 타입 체크**

```bash
pnpm tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src/components/SubagentRow.tsx src/components/SessionRow.tsx src/styles.css
git commit -m "feat(panel): 부모 세션 하위 서브에이전트 행 렌더 + lazy fetch"
```

---

## Task 10: Frontend — PreviewPane reply 입력창

**Files:**
- Modify: `src/components/PreviewPane.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: PreviewPane 에 pinned + reply state 추가**

`src/components/PreviewPane.tsx` 전체를 다음으로 교체:

```tsx
import { useEffect, useRef, useState } from "react";
import type { Session, SessionPreview } from "../types/session";
import { getSessionPreview, sendReply } from "../ipc/tauri";

interface Props {
  session: Session | null;
  focusReplyKey?: number;
  onUnpin?: () => void;
}

type SendState =
  | { kind: "idle" }
  | { kind: "sending" }
  | { kind: "done" }
  | { kind: "error"; message: string };

function classifyError(raw: string): { prefix: string; rest: string } {
  const m = raw.match(/^([a-z_]+):\s*(.*)$/i);
  if (m) return { prefix: m[1], rest: m[2] };
  return { prefix: "", rest: raw };
}

function errorHint(prefix: string): string {
  switch (prefix) {
    case "permission":
      return "Open System Settings > Privacy > Accessibility and enable aieye.";
    case "host_unsupported":
      return "Switch reply mode in Settings, or resume the session in a terminal.";
    case "cli_not_found":
      return "Install Claude Code CLI, or switch to Paste mode.";
    case "session_running":
      return "Wait for the response to finish, or switch to Paste mode.";
    case "session_not_running":
      return "Resume this session in a terminal first.";
    case "process_timeout":
      return "Response is still running in background — next refresh will pick it up.";
    default:
      return "Try again, or check the logs.";
  }
}

export function PreviewPane({ session, focusReplyKey, onUnpin }: Props) {
  const [preview, setPreview] = useState<SessionPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [replyText, setReplyText] = useState("");
  const [sendState, setSendState] = useState<SendState>({ kind: "idle" });
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!session) {
      setPreview(null);
      return;
    }
    let cancelled = false;
    setLoading(true);
    getSessionPreview(session.jsonl_path, session.cli)
      .then((p) => {
        if (!cancelled) setPreview(p);
      })
      .catch((err) => {
        console.error(err);
        if (!cancelled) setPreview(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [session?.jsonl_path, session?.cli]);

  // focusReplyKey 가 바뀌면 textarea 에 포커스
  useEffect(() => {
    if (focusReplyKey !== undefined) {
      textareaRef.current?.focus();
    }
  }, [focusReplyKey]);

  const onSend = async () => {
    if (!session) return;
    const text = replyText.trim();
    if (!text) return;
    setSendState({ kind: "sending" });
    try {
      await sendReply(session, text);
      setSendState({ kind: "done" });
      setReplyText("");
      setTimeout(() => setSendState({ kind: "idle" }), 1800);
    } catch (e) {
      const raw = String(e);
      const { prefix } = classifyError(raw);
      setSendState({
        kind: "error",
        message: `${raw}\n${errorHint(prefix)}`,
      });
    }
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      onSend();
    } else if (e.key === "Escape") {
      textareaRef.current?.blur();
    }
  };

  if (!session) {
    return (
      <div className="preview-pane empty-preview">
        <div className="hint">Hover a session to preview its conversation.</div>
      </div>
    );
  }

  const sending = sendState.kind === "sending";

  return (
    <div className="preview-pane">
      <div className="preview-header">
        <div className="preview-title">{session.title}</div>
        <div className="preview-meta">
          [{session.cli}] · {session.project_path ?? "-"}
          {onUnpin && (
            <button className="unpin-btn" onClick={onUnpin} title="Unpin">
              ✕
            </button>
          )}
        </div>
      </div>
      {loading && !preview && <div className="hint">Loading…</div>}
      {preview && preview.recent_turns.length === 0 && (
        <div className="hint">No recent messages.</div>
      )}
      {preview && preview.recent_turns.length > 0 && (
        <div className="turns">
          {preview.recent_turns.map((t, i) => (
            <div key={i} className={`turn ${t.role}`}>
              <div className="turn-role">{t.role === "user" ? "You" : "AI"}</div>
              <div className="turn-text">{t.text}</div>
            </div>
          ))}
        </div>
      )}
      <div className="reply-box">
        <textarea
          ref={textareaRef}
          className="reply-textarea"
          placeholder="Reply… (⌘↵ to send, Shift+↵ for newline)"
          value={replyText}
          onChange={(e) => setReplyText(e.target.value)}
          onKeyDown={onKeyDown}
          disabled={sending}
          rows={3}
        />
        <div className="reply-actions">
          <span className={`reply-status ${sendState.kind}`}>
            {sendState.kind === "sending" && "Sending…"}
            {sendState.kind === "done" && "Sent ✓"}
            {sendState.kind === "error" && <pre>{sendState.message}</pre>}
          </span>
          <button
            className="reply-send"
            onClick={onSend}
            disabled={sending || !replyText.trim()}
          >
            Send ⌘↵
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: App.tsx 에 pinned state**

`src/App.tsx` 의 컴포넌트 본문에 다음 state 추가 (기존 `hovered` 다음):

```typescript
  const [pinned, setPinned] = useState<Session | null>(null);
  const [focusKey, setFocusKey] = useState(0);

  const previewTarget = pinned ?? hovered;

  const pinSessionForReply = (s: Session) => {
    setPinned(s);
    setFocusKey((k) => k + 1);
  };
```

기존 `<PreviewPane session={hovered} />` 를 다음으로 교체:

```tsx
        <PreviewPane
          session={previewTarget}
          focusReplyKey={focusKey}
          onUnpin={pinned ? () => setPinned(null) : undefined}
        />
```

`SessionList` 컴포넌트 prop 에 `onPinReply={pinSessionForReply}` 도 함께 추가 (Task 11 에서 SessionList → SessionRow 로 prop drilling).

- [ ] **Step 3: 스타일 추가**

`src/styles.css` 끝에 추가:

```css
.preview-pane {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.preview-pane .turns {
  flex: 1;
  overflow-y: auto;
}
.preview-pane .unpin-btn {
  background: none;
  border: none;
  color: var(--text-muted, #9aa);
  cursor: pointer;
  margin-left: 6px;
  font-size: 11px;
}
.reply-box {
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  padding: 8px 10px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.reply-textarea {
  width: 100%;
  resize: vertical;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text, #ddd);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 6px 8px;
  font-size: 12px;
  font-family: inherit;
  box-sizing: border-box;
}
.reply-textarea:focus {
  outline: none;
  border-color: #6cb6ff;
}
.reply-textarea:disabled {
  opacity: 0.6;
}
.reply-actions {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}
.reply-status {
  flex: 1;
  font-size: 11px;
}
.reply-status.error pre {
  color: #e57373;
  white-space: pre-wrap;
  margin: 0;
  font-size: 10px;
}
.reply-status.done { color: #6cd07a; }
.reply-status.sending { color: #6cb6ff; }
.reply-send {
  background: #6cb6ff;
  color: #000;
  border: none;
  border-radius: 4px;
  padding: 4px 10px;
  font-size: 11px;
  cursor: pointer;
}
.reply-send:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
```

- [ ] **Step 4: 타입 체크**

```bash
pnpm tsc --noEmit
```

Expected: SessionList 가 `onPinReply` prop 모르므로 일시적 에러 가능. 다음 Task 에서 해소.

- [ ] **Step 5: Commit (다음 Task 와 묶어서 한 번에 처리 가능)**

이 Task 는 다음 Task 와 함께 컴파일되어야 하므로 commit 보류. Task 11 끝에서 함께 commit.

---

## Task 11: Frontend — SessionRow quick-reply 아이콘 + prop drilling

**Files:**
- Modify: `src/components/SessionList.tsx`
- Modify: `src/components/SessionRow.tsx`

- [ ] **Step 1: SessionList prop 에 onPinReply 추가**

`src/components/SessionList.tsx` 의 `Props` 인터페이스 끝에 추가:

```typescript
  onPinReply?: (session: Session) => void;
```

함수 시그니처 destructuring 에 추가:

```typescript
  onPinReply,
```

`<SessionRow ...>` JSX 호출에 prop 추가:

```tsx
                <SessionRow
                  key={`${s.cli}-${s.id}`}
                  session={s}
                  onHover={onHover}
                  onPinReply={onPinReply}
                  manageMode={manageMode}
                  selected={selected?.has(s.id)}
                  eligible={eligibleIds?.has(s.id) ?? false}
                  onToggleSelect={onToggleSelect}
                />
```

- [ ] **Step 2: SessionRow 에 ✉ 아이콘**

`src/components/SessionRow.tsx` 의 `Props` 인터페이스에 추가:

```typescript
  onPinReply?: (session: Session) => void;
```

함수 본문 destructuring 에 `onPinReply` 추가.

기존 `<button className="row-menu-btn" ...>` 라인 직전에 ✉ 아이콘 버튼 삽입:

```tsx
      <button
        className="row-reply-btn"
        data-row-action="reply"
        title="Reply in panel"
        onClick={(e) => {
          e.stopPropagation();
          onPinReply?.(session);
        }}
      >
        ✉
      </button>
```

`onClick` 핸들러의 `if ((e.target as HTMLElement).dataset.rowAction) return;` 가 이미 `data-row-action="reply"` 도 걸러주므로 별도 처리 불요.

- [ ] **Step 3: App.tsx 의 SessionList 호출에 onPinReply prop 추가**

`src/App.tsx` 의 `<SessionList ...>` JSX 에 추가:

```tsx
            onPinReply={pinSessionForReply}
```

- [ ] **Step 4: 스타일**

`src/styles.css` 끝에 추가:

```css
.row-reply-btn {
  background: none;
  border: none;
  color: var(--text-muted, #9aa);
  cursor: pointer;
  padding: 4px 6px;
  font-size: 13px;
  opacity: 0.6;
}
.session-row:hover .row-reply-btn {
  opacity: 1;
}
.row-reply-btn:hover {
  color: #6cb6ff;
}
```

- [ ] **Step 5: 타입 체크**

```bash
pnpm tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 6: Commit (Task 10 + 11 통합)**

```bash
git add src/components/PreviewPane.tsx src/components/SessionList.tsx \
        src/components/SessionRow.tsx src/App.tsx src/styles.css
git commit -m "feat(panel): in-panel reply 입력창 + 행 quick-reply 아이콘 + pin/unpin"
```

---

## Task 12: Frontend — Settings UI

**Files:**
- Modify: `src/components/SettingsMenu.tsx`

- [ ] **Step 1: SettingsMenu 에 group_by_repo + reply_mode**

`src/components/SettingsMenu.tsx` 전체를 다음으로 교체:

```tsx
import { useEffect, useState } from "react";
import type { ReplyMode, TerminalApp } from "../types/session";
import type { Settings } from "../types/settings";
import { useSettings } from "../hooks/useSettings";
import { listInstalledTerminals } from "../ipc/tauri";

const LABELS: Record<TerminalApp, string> = {
  terminal: "Terminal",
  iterm2: "iTerm2",
  alacritty: "Alacritty",
  kitty: "kitty",
};

const REPLY_LABELS: Record<ReplyMode, string> = {
  paste: "Paste to terminal",
  headless: "Headless (Claude only, idle)",
};

export function SettingsMenu() {
  const { settings, update } = useSettings();
  const [open, setOpen] = useState(false);
  const [installed, setInstalled] = useState<TerminalApp[] | null>(null);

  useEffect(() => {
    listInstalledTerminals().then(setInstalled).catch(() => setInstalled([]));
  }, []);

  if (!settings) return null;

  const options = installed ?? [];

  return (
    <div className="settings-menu">
      <button className="settings-toggle" onClick={() => setOpen((o) => !o)}>
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
              {options.length === 0 && (
                <option value={settings.preferred_terminal}>
                  {LABELS[settings.preferred_terminal]}
                </option>
              )}
              {options.map((t) => (
                <option key={t} value={t}>
                  {LABELS[t]}
                </option>
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
              onChange={(e) =>
                update({ recent_threshold_minutes: Number(e.target.value) || 60 })
              }
            />
          </label>
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={settings.group_by_repo}
              onChange={(e) => update({ group_by_repo: e.target.checked })}
            />
            <span>Group sessions by repo</span>
          </label>
          <fieldset className="settings-fieldset">
            <legend>Reply mode</legend>
            {(Object.keys(REPLY_LABELS) as ReplyMode[]).map((m) => (
              <label key={m} className="settings-radio">
                <input
                  type="radio"
                  name="reply_mode"
                  value={m}
                  checked={settings.reply_mode === m}
                  onChange={() => update({ reply_mode: m })}
                />
                <span>{REPLY_LABELS[m]}</span>
              </label>
            ))}
            <p className="settings-help">
              Paste mode requires Accessibility permission. Open{" "}
              <a
                href="#"
                onClick={(e) => {
                  e.preventDefault();
                  window.open?.(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                  );
                }}
              >
                System Settings → Privacy → Accessibility
              </a>
              .
            </p>
          </fieldset>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 스타일**

`src/styles.css` 끝에 추가:

```css
.settings-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
}
.settings-fieldset {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
  padding: 6px 10px;
  margin-top: 8px;
}
.settings-fieldset legend {
  font-size: 11px;
  opacity: 0.7;
  padding: 0 4px;
}
.settings-radio {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  margin: 4px 0;
}
.settings-help {
  font-size: 10px;
  opacity: 0.6;
  margin: 6px 0 0 0;
}
.settings-help a {
  color: #6cb6ff;
  text-decoration: underline;
}
```

- [ ] **Step 3: 타입 체크**

```bash
pnpm tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/SettingsMenu.tsx src/styles.css
git commit -m "feat(settings): group_by_repo 토글 + reply_mode radio + 권한 안내"
```

---

## Task 13: End-to-end verification

**Files:** (none — manual QA + smoke test)

- [ ] **Step 1: Rust 전체 테스트**

```bash
cd src-tauri && cargo test
```

Expected: 모든 테스트 통과 (기존 + 신규 subagent 테스트).

- [ ] **Step 2: 전체 build**

```bash
./build.sh
```

Expected: 디버그 빌드 성공, `.app` 생성.

- [ ] **Step 3: 앱 실행 & 수동 체크리스트**

```bash
./build.sh open
```

다음 체크리스트를 메뉴바 아이콘 → 패널에서 직접 확인:

- [ ] Group by repo 토글 on/off 즉시 반영
- [ ] 그룹 펼침/접힘 정상 동작
- [ ] Sub-agent 행이 들여쓰기 + 상태 dot 색 (running/completed/errored)
- [ ] Paste 모드: Terminal · iTerm2 양쪽에서 한글/이모지/멀티라인 전송 — 한글 "안녕하세요", "🚀 deploy" 등으로 직접 테스트
- [ ] Paste 모드: 클립보드가 원래 값으로 복원 (사전에 임의 텍스트 copy → 전송 → pbpaste 확인)
- [ ] Paste 모드: Accessibility 권한 미부여 상태에서 첫 시도 시 `permission:` prefix 에러 + 안내 노출
- [ ] Headless 모드: idle Claude 세션 → JSONL turn append 후 다음 폴링에 새 turn 보임 ⚠️ **Open Question #1 검증**: append 가 아니면 headless 모드는 v0.2 출시 보류, paste 만 출시. 검증 결과를 PR description 에 기록
- [ ] Headless 모드: Codex 세션 선택 → `host_unsupported:` 폴백 메시지
- [ ] Headless 모드: generating 중인 세션 → `session_running:` 메시지
- [ ] `(no project)` 그룹이 항상 맨 마지막
- [ ] PreviewPane reply 입력창 keybindings (`Cmd+Enter` 전송 / `Shift+Enter` 줄바꿈 / `Esc` blur)
- [ ] 행 ✉ 아이콘 클릭 → 해당 세션 PreviewPane pin + textarea focus
- [ ] 앱 재시작 후에도 Settings (`group_by_repo`, `reply_mode`) 영속

- [ ] **Step 4: Open Questions 답변 doc 업데이트**

수동 검증 결과를 spec 의 Open Questions 섹션에 인라인 노트로 추가:

```bash
# 예: spec doc 의 Open Questions #1 아래에 검증 결과 추가
```

- [ ] **Step 5: Final commit (필요 시)**

수동 QA 에서 발견된 micro-fix 가 있으면 별도 commit.

```bash
git commit -am "fix(panel): v0.2 수동 QA 미세 보정"
```

---

## Self-Review Checklist

### Spec coverage

| Spec section | Task |
|---|---|
| §3.1 Session 모델 확장 (repo_name, subagents) | Task 1 |
| §3.2 Sub-agent JSONL 파서 | Task 2 |
| §3.3 Settings (reply_mode, group_by_repo) | Task 1 |
| §3.4 IPC (send_reply, get_session_subagents) | Tasks 3, 6 |
| §3.5 Paste mechanism | Task 4 |
| §3.6 Headless mechanism | Task 5 |
| §4.1 그루핑/정렬 useMemo | Task 8 |
| §4.2 RepoGroupHeader / 컴포넌트 트리 | Tasks 8, 9 |
| §4.3 SubagentRow 표시 | Task 9 |
| §4.4 Reply 입력 UI (PreviewPane + quick-reply) | Tasks 10, 11 |
| §4.5 Reply state machine | Task 10 |
| §4.6 Settings UI | Task 12 |
| §5 에러 처리 (prefix 컨벤션) | Tasks 6, 10 |
| §6 성능 (lazy fetch, useMemo) | Tasks 3, 9, 8 |
| §7 테스트 & QA | Tasks 2, 13 |

### Placeholders / TODOs
없음. 모든 step 에 실제 코드/명령/예상 결과 포함.

### Type consistency
- `SubagentRow` / `SubagentState` 는 Task 1 (Rust) 과 Task 7 (TS) 에서 동일 필드명·타입 사용.
- `ReplyMode` 는 Task 1·6·7·12 에서 일관.
- `send_reply` 호출 시그니처: `{ session, text }` (Rust camelCase → TS camelCase 자동 매핑은 Tauri 기본 동작).

### Scope
4개 기능이 강하게 관련되어 (모두 SessionList 동선) 한 plan 으로 묶임. 분리 불요.
