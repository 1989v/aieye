# aieye — Session Grouping & In-Panel Reply Design

- **Date**: 2026-05-20
- **Status**: Draft
- **Author**: kgd
- **Targets**: v0.2 (post-v0.1.0)

---

## 1. Overview

aieye 의 4가지 기능 보완을 단일 릴리스로 묶는다.

1. **Repo grouping** — 세션 리스트를 `project_path` 의 마지막 세그먼트(=레포 루트 디렉토리명) 기준으로 묶고, 그룹은 각 그룹의 최신 활동 시간 desc 정렬.
2. **In-group sort** — 그룹 내부 세션을 `last_activity` desc 정렬.
3. **Sub-agent view** — Claude Code `Task` tool 로 호출된 서브에이전트들을 부모 세션 행 아래에 들여쓰기된 자식 행으로 노출.
4. **In-panel reply** — 터미널로 이동 없이 aieye 패널 안에서 세션에 직접 메시지를 전송. Paste(AppleScript) / Headless(`claude --resume --print`) 두 모드를 설정으로 토글, 기본은 Paste.

### 1.1 Goals

| # | Goal | 측정 |
|---|---|---|
| G1 | Repo grouping + in-group sort | 세션 100 개 이상 환경에서 그룹 헤더 + 행 정렬이 < 16ms 안에 리렌더 |
| G2 | Sub-agent rows | Task tool 2 개 이상 포함된 Claude 세션의 fixture JSONL 에서 sub-agent 2 행이 들여쓰기되어 보임 |
| G3 | Paste reply (Terminal · iTerm2) | 한글/이모지/멀티라인 텍스트가 정확히 전달, 클립보드 복원 |
| G4 | Headless reply (Claude only, idle) | `claude --resume <id> --print "<msg>"` 가 JSONL 에 turn 을 append → 다음 폴링 사이클에 표면화 |
| G5 | Settings 토글 | `group_by_repo`, `reply_mode` 영속 + 기존 사용자 설정 보존 |

### 1.2 Non-Goals (v0.2)

- Codex headless reply (Codex 는 v0.2 에서 paste 모드만)
- 서브에이전트 본문 PreviewPane 전용 영역 (후속 릴리스)
- 펼침/접힘 상태 영속화 (in-memory only)
- Cross-CLI 서브에이전트 (Codex 의 internal agent 개념과 통일된 모델은 후속 작업)
- Reply 히스토리/재전송 UI

---

## 2. Architecture

### 2.1 분리 경계 유지

| Layer | 책임 |
|---|---|
| Rust (`src-tauri/`) | JSONL 파싱, 프로세스/터미널 감지, AppleScript 실행, headless spawn, 설정 영속, IPC 노출 |
| React (`src/`) | 그루핑/정렬 (`useMemo`), 펼침/접힘 상태, reply 입력 UI, 설정 UI, 에러 메시지 분기 |

신규 코드는 모두 위 경계 안에서 결정.

### 2.2 신규/수정 파일

```
src-tauri/src/
├── sessions/model.rs              [수정] Session 에 repo_name, subagents 필드
├── parser/claude_jsonl.rs         [수정] Task tool_use/tool_result 파싱 추가
├── parser/subagent.rs             [신규] SubagentRow extractor
├── resume/paste.rs                [신규] AppleScript paste 디스패치
├── resume/headless.rs             [신규] claude --resume --print spawner
├── commands.rs                    [수정] send_reply, get_session_subagents 추가
├── settings/mod.rs                [수정] reply_mode, group_by_repo
└── tests/fixtures/                [신규] Task tool 포함 JSONL fixture 2 개

src/
├── types/session.ts               [수정] repo_name, subagents 미러링
├── types/settings.ts              [수정] reply_mode, group_by_repo 미러링
├── components/SessionList.tsx     [수정] group_by_repo 분기, RepoGroupHeader 삽입
├── components/RepoGroupHeader.tsx [신규] 펼침/접힘 + 메타
├── components/SessionRow.tsx      [수정] subagents 행 렌더, quick-reply 아이콘
├── components/SubagentRow.tsx     [신규] 들여쓰기 자식 행
├── components/PreviewPane.tsx     [수정] 하단 reply 입력창, pinned 상태
├── components/SettingsMenu.tsx    [수정] group_by_repo 토글, reply_mode radio
├── hooks/useSessions.ts           [수정] subagents lazy fetch (on expand)
└── ipc/tauri.ts                   [수정] sendReply, getSessionSubagents
```

---

## 3. Backend 변경

### 3.1 Session 모델 확장 (`src-tauri/src/sessions/model.rs`)

```rust
#[derive(Serialize)]
pub struct Session {
    // ... 기존 필드 ...
    pub repo_name: String,            // project_path 마지막 세그먼트, None → "(no project)"
    #[serde(default)]
    pub subagents: Vec<SubagentRow>,  // lazy fetch 전엔 empty
}

#[derive(Serialize, Clone)]
pub struct SubagentRow {
    pub id: String,                     // tool_use_id
    pub name: String,                   // Task 의 subagent_type
    pub description: Option<String>,    // Task 의 description
    pub state: SubagentState,
    pub last_text: Option<String>,      // tool_result 텍스트 truncated 120ch
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Serialize, Clone)]
pub enum SubagentState {
    Pending, Running, Completed, Errored,
}
```

`repo_name` 계산은 `list_sessions` 의 마지막 패스에서 `project_path.rsplit('/').next().unwrap_or("(no project)")` 로 채움.

### 3.2 Claude JSONL Sub-agent 파서 (`src-tauri/src/parser/subagent.rs` 신규)

기존 `claude_jsonl.rs` 의 라인 스트림 reader 재사용:

1. `message.content` 안에서 `type == "tool_use" && name == "Task"` 발견 시 `tool_use_id`, `input.subagent_type`, `input.description`, timestamp 캡쳐
2. 후속 라인 중 동일 `tool_use_id` 의 `type == "tool_result"` 가 나오면 종료 시점 + `content` 텍스트 캡쳐 + `is_error` 확인
3. State 결정 규칙:
   - tool_result 있음, `is_error == false` → `Completed`
   - tool_result 있음, `is_error == true` → `Errored`
   - tool_result 없음 + 부모 세션의 마지막 메시지가 `stop_reason == "end_turn"` → `Errored` (orphan)
   - tool_result 없음 + 부모 세션이 generating 중 → `Running`
   - 그 외 → `Pending`
4. `last_text`: `tool_result.content` 가 string 이면 그대로 truncate; array 면 `text` 만 concat 후 truncate

성능 정책:

- list_sessions 응답에는 **running 세션만** 사전 파싱한 `subagents` 를 포함 (기존 inline_preview 정책과 동일 — 최대 N=20 세션 한정).
- 나머지(idle/recent) 세션은 빈 `subagents: []` 로 응답.
- FE 가 세션을 펼치면 `get_session_subagents(jsonl_path)` 로 on-demand 풀 파싱.

이렇게 하면 (a) 활성 작업이 첫 화면에서 즉시 보이고 (b) 전체 latency 는 영향 없음.

### 3.3 Settings (`src-tauri/src/settings/mod.rs`)

```rust
#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    // ... 기존 ...
    #[serde(default = "default_reply_mode")]
    pub reply_mode: ReplyMode,
    #[serde(default = "default_group_by_repo")]
    pub group_by_repo: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum ReplyMode { Paste, Headless }
fn default_reply_mode() -> ReplyMode { ReplyMode::Paste }
fn default_group_by_repo() -> bool { true }
```

`#[serde(default)]` 로 기존 사용자 설정 파일과 forward-compatible.

### 3.4 IPC 명령 (`src-tauri/src/commands.rs`)

```rust
#[tauri::command]
pub async fn send_reply(
    session: Session,
    text: String,
    state: State<'_, SharedSettings>,
) -> Result<(), String>;

#[tauri::command]
pub async fn get_session_subagents(
    jsonl_path: String,
    cli: CliKind,
) -> Result<Vec<SubagentRow>, String>;
```

`send_reply` 디스패치 로직:

```
if cli != Claude && reply_mode == Headless:
    return Err("host_unsupported: Codex headless not yet supported")
if running.activity == Generating && reply_mode == Headless:
    return Err("session_running: headless requires idle session")

match reply_mode:
    Paste    → resume::paste::send(&session, &text)
    Headless → resume::headless::send(&session, &text)
```

### 3.5 Paste mechanism (`src-tauri/src/resume/paste.rs`)

```rust
pub async fn send(session: &Session, text: &str) -> Result<(), String> {
    let running = session.running.as_ref()
        .ok_or("session_not_running: paste requires running session")?;

    match running.host_kind {
        HostKind::VsCode | HostKind::JetBrains =>
            return Err("host_unsupported: paste only supported on Terminal/iTerm2/Alacritty/Kitty"),
        _ => {}
    }

    let backup = backup_clipboard().await.ok();      // best-effort
    set_clipboard(text).await?;
    activate_and_paste(running).await
        .map_err(|e| format!("permission: AppleScript failed ({e}). Grant Accessibility access."))?;
    sleep(Duration::from_millis(120)).await;
    if let Some(b) = backup { let _ = set_clipboard(&b).await; }
    Ok(())
}
```

- `activate_and_paste` 는 `host_kind` 별 osascript:
  - Terminal/iTerm2 : `tell application "Terminal" to activate` → `tell process "Terminal" to keystroke "v" using command down` → `keystroke return`
  - Alacritty/Kitty : bundle id 로 activate, 동일 Cmd+V/return
- 클립보드 백업/복원은 `pbpaste` / `pbcopy` subprocess. 실패해도 send 자체는 성공으로 간주 (warning 로그)

### 3.6 Headless mechanism (`src-tauri/src/resume/headless.rs`)

```rust
pub async fn send(session: &Session, text: &str) -> Result<(), String> {
    let bin = which::which("claude")
        .map_err(|_| "cli_not_found: claude binary missing")?;

    let mut child = tokio::process::Command::new(bin)
        .args(["--resume", &session.id, "--print", text])
        .stdout(Stdio::null()).stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("process_failed: {e}"))?;

    // 30s 까지만 기다리고, 초과 시 detach (kill X)
    let _ = tokio::time::timeout(Duration::from_secs(30), child.wait()).await;
    Ok(())
}
```

다음 list_sessions 폴링이 새 turn 을 자연스럽게 픽업 (기존 watcher 재사용).

> **Open Question (사전 검증 필요)**: `claude --resume <id> --print "<msg>"` 가 동일 JSONL 에 turn 을 append 하는가, 아니면 새 sub-conversation 을 만드는가? v0.2 implementation 시작 전 수동 검증. append 가 아니면 v0.2 에서 headless 모드 비활성 + paste 만 출시.

---

## 4. Frontend 변경

### 4.1 그루핑/정렬 (`src/App.tsx`)

```typescript
const grouped = useMemo<RepoGroup[]>(() => {
  if (!settings.group_by_repo) return [{ repoName: "", sessions: filtered, latestActivity: "" }];
  const map = new Map<string, Session[]>();
  for (const s of filtered) {
    const key = s.repo_name || "(no project)";
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(s);
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
}, [filtered, settings.group_by_repo]);

// 참고: FE 의 Session/Settings 타입은 기존 컨벤션대로 snake_case 유지 (예: `project_path`, `last_activity`, `reply_mode`, `group_by_repo`). Tauri serde 기본 동작과 정합.
```

### 4.2 컴포넌트 트리

```
SessionList
└── RepoGroup[]
    ├── RepoGroupHeader (collapsable)
    │   ├── caret icon
    │   ├── repo name
    │   ├── session count badge
    │   └── relative time of latestActivity
    └── for each Session:
        ├── SessionRow
        │   ├── 기존 컨텐츠
        │   └── reply ✉ 아이콘 (우측 오버플로우 옆)
        └── for each Subagent:
            └── SubagentRow (들여쓰기 16px, 좌측 ↳ glyph)
```

펼침/접힘 상태:

```typescript
const [collapsedRepos, setCollapsedRepos] = useState<Set<string>>(new Set());
```

v0.2 는 in-memory only. v0.3 에서 영속화 검토.

### 4.3 SubagentRow 표시

```
  ↳ ● Explore                                    5m ago
      "Explore SessionList.tsx and surrounding..."
```

- 좌측 dot 색: running=blue blink, completed=green, errored=red, pending=gray
- 가운데: name + description 1줄 truncate (ellipsis)
- 우측: 완료된 행은 absolute 시간 (`5m ago`), running 행은 elapsed (`0:23`)
- 클릭: 부모 세션을 PreviewPane 에 pin (서브에이전트 본문 전용 영역은 v0.2 스코프 밖, 섹션 1.2 참고)

### 4.4 Reply 입력 UI

**(a) PreviewPane 하단 고정 입력창** (`PreviewPane.tsx`)
- `pinned: Session | null` 이 있을 때만 입력창 렌더
- `<textarea>` (auto-grow, max 6 lines) + Send 버튼
- 키바인딩: `Cmd+Enter` 전송, `Shift+Enter` 줄바꿈, `Esc` blur

```
┌─────────────────────────────────────────────┐
│ (recent turns preview ...)                  │
│                                             │
├─────────────────────────────────────────────┤
│ [Reply to "Fix login bug"...]              │
│                                             │
│                          [Send ⌘↵]         │
└─────────────────────────────────────────────┘
```

**(b) 행별 quick-reply 아이콘** (`SessionRow.tsx`)
- 우측 오버플로우 메뉴 (`⋯`) 옆에 ✉ 아이콘
- 클릭 → 해당 세션을 `pinned` 으로 set + PreviewPane 의 textarea 에 focus

두 진입점이 같은 단일 입력창에 모이므로 상태 관리는 `pinned + replyText + sendingState` 트리오면 충분.

### 4.5 Reply state machine (PreviewPane 로컬)

```
idle ──(submit)──> sending ──(ok)──> done(2s) ──> idle
                       │
                       └──(err)──> error(메시지 inline) ──(retry)──> sending
```

- `sending` 동안 textarea/Send 모두 disabled, spinner
- `done` 후 자동으로 textarea clear, PreviewPane 폴링이 새 turn 픽업

### 4.6 Settings UI (`SettingsMenu.tsx`)

```
[ ] Group sessions by repo
Reply mode:
  ● Paste to terminal (default)
  ○ Headless — Claude only, idle sessions
    ℹ Paste mode requires Accessibility permission. [Open System Settings]
```

권한 미부여 감지 → 상단 warning 배너 + CTA 버튼 (`open "x-apple.systempreferences:..."`).

---

## 5. 에러 처리

`send_reply` 의 `Result<(), String>` 에러는 prefix 컨벤션으로 FE 분기:

| Prefix | 의미 | FE 액션 |
|---|---|---|
| `permission:` | Accessibility 권한 부재 | warning 배너 + Open Settings CTA |
| `host_unsupported:` | host_kind 미지원 또는 cli/mode 조합 미지원 | "다른 모드로 전환 또는 터미널로 이동" |
| `cli_not_found:` | `claude` 바이너리 없음 | "Claude Code CLI 미설치. paste 로 폴백?" |
| `session_running:` | headless 인데 generating 중 | "응답 중. paste 로 폴백?" |
| `session_not_running:` | paste 인데 running 정보 없음 | "이 세션은 활성 터미널이 없습니다. 먼저 resume 하세요." |
| `process_failed:` | spawn/AppleScript 기타 실패 | inline 에러 + 재시도 |
| `process_timeout:` | headless 30s 초과 | "백그라운드 진행 중. 곧 갱신됩니다." |

---

## 6. 성능

| 작업 | 전략 | 예상 비용 |
|---|---|---|
| Sub-agent 풀 파싱 | active 세션은 첫 응답 포함 (최대 20), 나머지는 펼침 시 lazy | 첫 list_sessions latency 영향 무시 가능 (기존 inline_preview 와 동일 비용 모델) |
| 그루핑/정렬 | FE `useMemo`, 의존 = `filtered + group_by_repo` | < 10ms / 1000 세션 |
| Paste | osascript 1 회 + 클립보드 2 회 | < 200ms |
| Headless | spawn + claude 응답 대기 | 외부 의존 (sec 단위) |

---

## 7. 테스트 & QA

### 7.1 Rust unit tests
- `parser::subagent::extract` — Task 페어링, error, orphan 케이스
- 손상된 JSON 라인 skip
- `repo_name` 추출 edge cases (`/`, 빈 path, trailing slash)

### 7.2 Rust integration tests
- Fixture A: Task 없음 → empty subagents
- Fixture B: Task 2 개 (Explore + general-purpose, 둘 다 Completed)
- Fixture C: orphan Task (tool_result 없음, stop_reason=end_turn) → Errored

### 7.3 FE 테스트
- v0.2 에선 자동화 미도입. 수동 QA 체크리스트로 갈음.

### 7.4 수동 QA 체크리스트
- [ ] Group by repo 토글 on/off 즉시 반영
- [ ] 그룹 펼침/접힘 정상 동작
- [ ] Sub-agent 행 들여쓰기 + 상태 dot 색
- [ ] Paste 모드: Terminal · iTerm2 양쪽에서 한글/이모지/멀티라인 전송
- [ ] Paste 모드: 클립보드가 원래 값으로 복원
- [ ] Paste 모드: Accessibility 권한 미부여 시 warning 노출
- [ ] Headless 모드: idle Claude 세션 → JSONL turn append 확인
- [ ] Headless 모드: Codex 선택 → `host_unsupported` 폴백 안내
- [ ] Headless 모드: running 세션 → `session_running` 안내
- [ ] `(no project)` 그룹이 항상 맨 마지막
- [ ] PreviewPane reply 입력창 keybindings (`Cmd+Enter`/`Shift+Enter`/`Esc`)
- [ ] 행별 ✉ 아이콘 → PreviewPane pin + focus
- [ ] Settings 변경 후 앱 재시작에도 영속

---

## 8. 마이그레이션 & 호환성

- Settings 신규 필드는 `#[serde(default)]` → 기존 설정 파일 손실 없음.
- Session 타입은 `repo_name` / `subagents` 추가. FE/BE 동시 배포 가정 (Tauri 단일 바이너리).
- 기존 IPC (`list_sessions`, `resume_session`, ...) signature 무변경.
- `Group by repo`/`reply_mode` 둘 다 기본값이 "유저가 처음 켰을 때 보던 동작"에서 약간 달라짐(`group_by_repo=true`):
  - 단일 그룹만 있는 경우엔 헤더가 conspicuous 하지 않도록 1 그룹일 때는 헤더 hide 검토 (v0.2 에서 결정).

---

## 9. Open Questions

1. **Headless append 가설 검증** — `claude --resume <id> --print "<msg>"` 가 동일 JSONL 에 append 인지 사전 수동 테스트. append 아니면 headless 모드 출시 보류.
2. **Sub-agent 부분 데이터** — running 세션의 in-flight Task tool_result 가 stream 중일 때 partial JSON 라인 처리. 기존 `claude_jsonl.rs` 가 EOF 라인 skip 하므로 다음 폴링까지 자연 해결로 본다 (검증 필요).
3. **Codex headless** — Codex CLI 가 `--print` 류 비대화 모드를 지원하는지. v0.2 에서는 paste 만, v0.3 에서 재검토.
4. **단일 그룹일 때 헤더 렌더 정책** — 데모/스크린샷 자연스러움 위해 1 그룹이면 헤더 숨김 vs 항상 노출. 구현 시 결정.

---

## 10. Out of Scope (명시)

- Reply 히스토리, 재전송 UI, 초안 저장
- 서브에이전트 본문 PreviewPane 확장
- 펼침/접힘 영속화
- iPad/iPhone (이전 v0.1 doc 의 non-goals 유지)
