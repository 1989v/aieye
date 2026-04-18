# aieye Plan 1 — Skeleton + ClaudeAdapter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tauri v2 메뉴바 스켈레톤 + Claude Code 세션 스캐너를 구현. 이 plan 완료 시점에 메뉴바 아이콘 클릭 → 로컬 `~/.claude/projects` 세션 리스트(타이틀/cwd/mtime)가 실제 표시됨.

**Architecture:** `pnpm create tauri-app` 로 React+TS+Vite 스캐폴드, Rust 쪽에 `sessions/` 모듈에서 Adapter 트레이트 + ClaudeAdapter 구현. Tauri `#[command]` 로 React 에 세션 배열 전달.

**Tech Stack:** Tauri v2, Rust (tokio/serde/anyhow/chrono), React 19 + TypeScript + Vite, pnpm.

---

## File Structure (Plan 1 산출물)

```
aieye/
├── package.json                       # pnpm + tauri scripts
├── pnpm-lock.yaml
├── tsconfig.json
├── vite.config.ts
├── index.html
├── src/                               # React frontend
│   ├── main.tsx                       # entry
│   ├── App.tsx                        # root component
│   ├── components/
│   │   └── SessionList.tsx
│   ├── hooks/
│   │   └── useSessions.ts
│   ├── ipc/
│   │   └── tauri.ts                   # typed invoke wrapper
│   ├── types/
│   │   └── session.ts                 # Session / CliKind mirror of Rust
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/                         # default tauri icon set
│   └── src/
│       ├── main.rs                    # entry, tray, window
│       ├── tray.rs                    # TrayIconBuilder setup
│       ├── commands.rs                # #[tauri::command] list_sessions
│       ├── sessions/
│       │   ├── mod.rs                 # pub use
│       │   ├── model.rs               # Session, CliKind, SessionState
│       │   ├── adapter.rs             # SessionAdapter trait
│       │   └── claude.rs              # ClaudeAdapter
│       └── parser/
│           ├── mod.rs
│           ├── project_slug.rs        # slug → path decoder
│           └── claude_jsonl.rs        # JSONL header parser
└── Tests/                             # Rust 테스트는 #[cfg(test)] 인라인 + tests/ integration
    └── (fixture files as needed in tests/fixtures/)
```

---

## Task 1: Tauri 프로젝트 스캐폴드

**Files:** 전체 신규 생성 (pnpm create)

- [ ] **Step 1: tooling 확인**

Run:
```bash
node --version   # v18+ 필요
pnpm --version   # v8+ 필요 (없으면: brew install pnpm)
cargo --version  # 1.70+ (없으면: brew install rustup → rustup default stable)
```

Expected: 세 가지 모두 버전 출력. 하나라도 없으면 해당 설치 후 재시도.

- [ ] **Step 2: aieye 디렉터리로 이동**

Run:
```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
ls -la  # LICENSE, README.md, .gitignore, docs/ 가 보여야 함
```

- [ ] **Step 3: Tauri 프로젝트 일시 scaffolding (다른 위치에서 만들고 파일만 복사)**

aieye/ 디렉터리는 이미 git 초기화 + docs 가 있으므로, 임시 위치에서 생성 후 필요한 파일만 옮긴다.

```bash
cd /tmp && rm -rf aieye-scaffold
pnpm create tauri-app@latest aieye-scaffold \
  --manager pnpm \
  --template react-ts \
  --identifier com.1989v.aieye \
  --app-name aieye
```

Expected: `/tmp/aieye-scaffold/` 생성. 중간에 질문 안 뜨고 자동 진행.

- [ ] **Step 4: scaffold 에서 필요한 파일만 aieye 로 복사**

```bash
cd /tmp/aieye-scaffold

# 루트 설정 파일
cp package.json tsconfig.json tsconfig.node.json vite.config.ts index.html /Users/gideok-kwon/IdeaProjects/aieye/

# src/ (React)
mkdir -p /Users/gideok-kwon/IdeaProjects/aieye/src
cp -R src/* /Users/gideok-kwon/IdeaProjects/aieye/src/

# src-tauri/ (Rust)
mkdir -p /Users/gideok-kwon/IdeaProjects/aieye/src-tauri
cp -R src-tauri/* /Users/gideok-kwon/IdeaProjects/aieye/src-tauri/

# 필요 없는 파일 정리
rm -f /Users/gideok-kwon/IdeaProjects/aieye/src/App.css
rm -f /Users/gideok-kwon/IdeaProjects/aieye/src/assets/react.svg
```

- [ ] **Step 5: pnpm install + tauri info 확인**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
pnpm install 2>&1 | tail -5
pnpm tauri info 2>&1 | head -20
```

Expected: `pnpm install` 성공 (node_modules/ 생성). `tauri info` 에서 `tauri: 2.x.x`, `rustc: 1.xx` 등 정상 출력.

- [ ] **Step 6: 첫 빌드 + 실행 확인 (dev 모드)**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
pnpm tauri dev 2>&1 &
TAURI_PID=$!
sleep 30  # 초기 Rust 컴파일 1-2분 걸릴 수 있음. 안전하게 30초만 기다리고 프로세스 확인.
pgrep -f "target/debug/aieye" && echo "aieye debug binary running"
kill $TAURI_PID 2>/dev/null
wait 2>/dev/null
```

Expected: 바탕화면에 Tauri 기본 "Welcome to Tauri" 창이 뜸, dock 에 아이콘 출현.
현재 시점에는 **메뉴바 앱 아님 (일반 창 앱)**. Task 2-3 에서 메뉴바로 전환.

- [ ] **Step 7: .gitignore 보강 + 첫 커밋**

Append to `.gitignore` (이미 Rust/Node 섹션 있음 — 중복 없이 추가):

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
grep -q "^pnpm-debug.log$" .gitignore || echo "pnpm-debug.log" >> .gitignore
grep -q "^src-tauri/target/$" .gitignore || echo "src-tauri/target/" >> .gitignore
```

Commit:
```bash
git add -A
git commit -m "feat: Tauri v2 + React + TS 스캐폴드 (pnpm create tauri-app)"
```

---

## Task 2: tauri.conf.json — 메뉴바 앱 설정

**Files:**
- Modify: `src-tauri/tauri.conf.json`

메뉴바 앱으로 전환: dock 아이콘 숨김(LSUIElement), 기본 창은 숨김 + frameless 패널 윈도우.

- [ ] **Step 1: tauri.conf.json 확인**

```bash
cat /Users/gideok-kwon/IdeaProjects/aieye/src-tauri/tauri.conf.json
```

Expected: `productName: "aieye"`, `identifier: "com.1989v.aieye"`, `app.windows[0]` 존재.

- [ ] **Step 2: tauri.conf.json 수정**

Overwrite `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/tauri.conf.json` with:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "aieye",
  "version": "0.1.0",
  "identifier": "com.1989v.aieye",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "panel",
        "title": "aieye",
        "width": 360,
        "height": 520,
        "minWidth": 360,
        "minHeight": 320,
        "decorations": false,
        "transparent": false,
        "alwaysOnTop": true,
        "resizable": false,
        "visible": false,
        "skipTaskbar": true,
        "shadow": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "app",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "macOS": {
      "minimumSystemVersion": "13.0",
      "entitlements": null,
      "exceptionDomain": "",
      "frameworks": [],
      "providerShortName": null,
      "signingIdentity": null
    }
  }
}
```

주요 포인트:
- `visible: false` — 시작 시 창 숨김 (트레이 클릭 시 show)
- `decorations: false` — 타이틀바 없음 (메뉴바 팝오버 느낌)
- `skipTaskbar: true` — Dock/태스크바 노출 안 함
- `alwaysOnTop: true` — 포커스 잃어도 일시적으로 위

- [ ] **Step 3: Info.plist 템플릿에 LSUIElement 추가**

`src-tauri/Info.plist` 파일을 생성해 Tauri 가 번들에 포함하도록:

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/Info.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
```

그리고 `tauri.conf.json` 의 `bundle.macOS` 에 연결 — 기존 `macOS` 섹션을 다음으로 교체:

```json
    "macOS": {
      "minimumSystemVersion": "13.0",
      "entitlements": null,
      "frameworks": [],
      "providerShortName": null,
      "signingIdentity": null,
      "infoPlist": "Info.plist"
    }
```

- [ ] **Step 4: 빌드 확인**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
pnpm tauri build --debug 2>&1 | tail -10
```

Expected: `Finished`, Dock 에 아이콘 출현 X (LSUIElement 적용). 아직 트레이도 없어서 실행해도 UI 안 보임 (Task 3 에서 추가).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/Info.plist
git commit -m "feat(tauri): LSUIElement + frameless panel window 설정"
```

---

## Task 3: Rust — 트레이 아이콘 + 패널 토글

**Files:**
- Modify: `src-tauri/Cargo.toml` (feature 추가)
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Cargo.toml 에 features 추가**

Edit `src-tauri/Cargo.toml` — `[dependencies]` 섹션의 `tauri` 라인을 다음으로 교체:

```toml
tauri = { version = "2", features = ["tray-icon", "macos-private-api"] }
```

추가 dep:
```toml
tokio = { version = "1", features = ["full"] }
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 2: tray.rs 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/tray.rs`:

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Manager,
};

pub fn build_tray(app: &App) -> tauri::Result<()> {
    let quit_item = MenuItem::with_id(app, "quit", "Quit aieye", true, Some("cmd+q"))?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    let _tray = TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("panel") {
                    if win.is_visible().unwrap_or(false) {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
```

- [ ] **Step 3: main.rs 재작성**

Overwrite `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/main.rs`:

```rust
// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aieye=info".into()),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            tray::build_tray(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 빌드 + 스모크 테스트**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
pnpm tauri build --debug 2>&1 | tail -5
```

Expected: Build complete.

```bash
open src-tauri/target/debug/bundle/macos/aieye.app
sleep 2
pgrep -f aieye.app/Contents/MacOS/aieye && echo "aieye running"
```

Expected: 메뉴바 우측에 기본 Tauri 아이콘 출현 (임시 default icon). 클릭 시 아직 창 내용은 React 기본이지만 토글 동작 확인.

```bash
pkill -f "aieye.app/Contents/MacOS/aieye"
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/main.rs src-tauri/src/tray.rs
git commit -m "feat(rust): 트레이 아이콘 + panel window 토글"
```

---

## Task 4: React — 기본 패널 UI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/main.tsx`
- Create: `src/styles.css`

- [ ] **Step 1: styles.css 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/styles.css`:

```css
* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html, body, #root {
  height: 100%;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', sans-serif;
  font-size: 13px;
  color: #1d1d1f;
  background: #f5f5f7;
}

@media (prefers-color-scheme: dark) {
  html, body, #root { background: #1e1e1e; color: #f5f5f7; }
}

.app {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.header {
  padding: 12px 16px;
  font-weight: 600;
  border-bottom: 1px solid rgba(0, 0, 0, 0.08);
}

.empty {
  padding: 24px 16px;
  color: #888;
  text-align: center;
}
```

- [ ] **Step 2: App.tsx 재작성**

Overwrite `/Users/gideok-kwon/IdeaProjects/aieye/src/App.tsx`:

```tsx
import "./styles.css";

export default function App() {
  return (
    <div className="app">
      <div className="header">👁 aieye</div>
      <div className="empty">
        Loading sessions…<br />
        (will list your Claude Code + Codex sessions)
      </div>
    </div>
  );
}
```

- [ ] **Step 3: main.tsx 확인 (대부분 scaffold 그대로)**

Ensure `/Users/gideok-kwon/IdeaProjects/aieye/src/main.tsx` is:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 4: 빌드 + 확인**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
pnpm tauri build --debug 2>&1 | tail -3
pkill -f "aieye.app/Contents/MacOS/aieye" 2>/dev/null
open src-tauri/target/debug/bundle/macos/aieye.app
sleep 2
pgrep -f aieye.app/Contents/MacOS/aieye
```

트레이 클릭 → "👁 aieye" 헤더 + "Loading sessions…" 메시지가 보이는 창이 뜨면 OK.

```bash
pkill -f "aieye.app/Contents/MacOS/aieye"
```

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/main.tsx src/styles.css
git commit -m "feat(ui): 기본 패널 — 헤더 + 로딩 placeholder"
```

---

## Task 5: Rust — Session / CliKind 모델

**Files:**
- Create: `src-tauri/src/sessions/mod.rs`
- Create: `src-tauri/src/sessions/model.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: sessions/model.rs 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/model.rs`:

```rust
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
```

- [ ] **Step 2: sessions/mod.rs 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/mod.rs`:

```rust
pub mod model;

pub use model::{CliKind, Session, SessionState};
```

- [ ] **Step 3: main.rs 에 모듈 등록**

Edit `src-tauri/src/main.rs` — `mod tray;` 아래 추가:

```rust
mod sessions;
```

- [ ] **Step 4: 빌드 확인**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3
```

Expected: `Finished`, warning 없음.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/sessions/ src-tauri/src/main.rs
git commit -m "feat(sessions): Session / CliKind / SessionState 모델"
```

---

## Task 6: Rust — 프로젝트 slug 디코더 + 테스트

**Files:**
- Create: `src-tauri/src/parser/mod.rs`
- Create: `src-tauri/src/parser/project_slug.rs`
- Modify: `src-tauri/src/main.rs`

`~/.claude/projects/-Users-gideok-kwon-IdeaProjects-msa` → `/Users/gideok-kwon/IdeaProjects/msa` 복원.

- [ ] **Step 1: 실패 테스트 작성 (TDD)**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/parser/project_slug.rs`:

```rust
use std::path::PathBuf;

/// `~/.claude/projects/{slug}` 의 슬러그를 원래 경로로 복원.
/// Claude Code 는 경로의 `/` 를 `-` 로 치환해 디렉터리명으로 사용.
/// 첫 문자도 `/` 였으므로 슬러그는 `-` 로 시작.
pub fn decode_project_slug(slug: &str) -> Option<PathBuf> {
    if !slug.starts_with('-') {
        return None;
    }
    // 앞쪽 `-` 한 개만 `/` 로. 나머지는 전부 `/` 로 치환.
    // 단, `-` 가 원래 경로에 있었을 수도 있으나 Claude Code 는 그 경우 구분 안 함 (정보 손실).
    // 가장 단순하게: 모든 `-` → `/` 로 복원.
    let decoded = slug.replace('-', "/");
    Some(PathBuf::from(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_simple_path() {
        let path = decode_project_slug("-Users-gideok-kwon-IdeaProjects-msa");
        assert_eq!(
            path,
            Some(PathBuf::from("/Users/gideok/kwon/IdeaProjects/msa"))
        );
    }

    #[test]
    fn returns_none_for_non_slug() {
        assert_eq!(decode_project_slug("no-leading-dash"), None);
    }

    #[test]
    fn empty_slug_returns_none() {
        assert_eq!(decode_project_slug(""), None);
    }
}
```

**주의**: 위 테스트는 `gideok-kwon` 이 `gideok/kwon` 으로 잘못 복원됨을 보여줌 — Claude Code 슬러그 알고리즘의 정보 손실.
실제 홈 디렉터리와 비교해 heuristic 복구 필요. Step 3 에서 보강.

- [ ] **Step 2: parser/mod.rs 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/parser/mod.rs`:

```rust
pub mod project_slug;

pub use project_slug::decode_project_slug;
```

- [ ] **Step 3: main.rs 에 모듈 등록 + 테스트 실행**

Edit `src-tauri/src/main.rs`:

```rust
mod tray;
mod sessions;
mod parser;
```

Run:
```bash
cd /Users/gideok-kwon/IdeaProjects/aieye/src-tauri
cargo test --lib decode_project_slug 2>&1 | tail -20
```

Expected: 테스트 중 1~2개는 통과, `decodes_simple_path` 는 실제 `gideok-kwon` → `gideok/kwon` 문제로 실패할 수 있음. 우리가 의도적으로 검증하려는 동작.

- [ ] **Step 4: 구현 개선 — `$HOME` 기준 prefix 매칭**

Replace the `decode_project_slug` function with home-aware version:

```rust
use std::path::PathBuf;

pub fn decode_project_slug(slug: &str) -> Option<PathBuf> {
    decode_with_home(slug, std::env::var("HOME").ok().as_deref())
}

pub fn decode_with_home(slug: &str, home: Option<&str>) -> Option<PathBuf> {
    if !slug.starts_with('-') {
        return None;
    }
    // 단순 전치환: `-` → `/`
    let naive = slug.replace('-', "/");

    // HOME 이 있으면 heuristic: `/Users/` 이후의 `-` 를 복원 시도.
    // 예) /Users/gideok/kwon/... 인데 HOME 이 /Users/gideok-kwon 이면
    //     "gideok/kwon" 을 "gideok-kwon" 으로 복구.
    if let Some(home) = home {
        let home_slug = home.replace('/', "-");  // "Users-gideok-kwon"
        if slug.starts_with(&home_slug) {
            // 슬러그에서 home 부분만 원래 home 으로 치환
            let rest = &slug[home_slug.len()..]; // "-IdeaProjects-msa"
            let rest_path = rest.replace('-', "/");
            return Some(PathBuf::from(format!("{home}{rest_path}")));
        }
    }

    Some(PathBuf::from(naive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_with_home_preserves_home_dashes() {
        let path = decode_with_home(
            "-Users-gideok-kwon-IdeaProjects-msa",
            Some("/Users/gideok-kwon"),
        );
        assert_eq!(
            path,
            Some(PathBuf::from("/Users/gideok-kwon/IdeaProjects/msa"))
        );
    }

    #[test]
    fn decodes_without_home_falls_back_to_naive() {
        let path = decode_with_home(
            "-Users-gideok-kwon-IdeaProjects-msa",
            None,
        );
        assert_eq!(
            path,
            Some(PathBuf::from("/Users/gideok/kwon/IdeaProjects/msa"))
        );
    }

    #[test]
    fn returns_none_for_non_slug() {
        assert_eq!(decode_with_home("no-leading-dash", None), None);
    }
}
```

- [ ] **Step 5: 테스트 통과 확인**

Run:
```bash
cd /Users/gideok-kwon/IdeaProjects/aieye/src-tauri
cargo test --lib project_slug 2>&1 | tail -10
```

Expected: 3/3 통과.

- [ ] **Step 6: Commit**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
git add src-tauri/src/parser/ src-tauri/src/main.rs
git commit -m "feat(parser): project slug → path 디코더 (HOME heuristic 포함)"
```

---

## Task 7: Rust — Claude JSONL 헤더 파서

**Files:**
- Create: `src-tauri/src/parser/claude_jsonl.rs`
- Modify: `src-tauri/src/parser/mod.rs`
- Create: `src-tauri/tests/fixtures/sample-claude.jsonl`

첫 `type:"user"` 메시지 + cwd + gitBranch + timestamp 를 파일 처음 ~20 라인 안에서 추출.

- [ ] **Step 1: fixture 파일 준비**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/tests/fixtures/sample-claude.jsonl`:

```json
{"type":"permission-mode","permissionMode":"bypassPermissions","sessionId":"abc-123"}
{"type":"file-history-snapshot","messageId":"msg-1","snapshot":{"messageId":"msg-1","trackedFileBackups":{}}}
{"parentUuid":null,"type":"user","message":{"role":"user","content":"Help me refactor the auth module"},"cwd":"/Users/kgd/IdeaProjects/aieye","sessionId":"abc-123","gitBranch":"main","timestamp":"2026-04-18T12:00:00.000Z"}
{"parentUuid":"msg-1","type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Sure, let me read the current auth code..."}]},"sessionId":"abc-123","timestamp":"2026-04-18T12:00:02.000Z"}
```

- [ ] **Step 2: 파서 구현**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/parser/claude_jsonl.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

/// 세션 헤더(첫 유저 메시지)에서 추출한 메타데이터.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionHeader {
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub first_timestamp: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct RawLine<'a> {
    #[serde(rename = "type")]
    line_type: Option<&'a str>,
    message: Option<RawMessage<'a>>,
    cwd: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    timestamp: Option<String>,
}

#[derive(Deserialize)]
struct RawMessage<'a> {
    role: Option<&'a str>,
    content: Option<serde_json::Value>,
}

/// JSONL 파일의 처음 MAX_LINES 줄 안에서 첫 user 메시지를 찾아 헤더 반환.
pub fn read_session_header(path: &Path) -> anyhow::Result<Option<SessionHeader>> {
    const MAX_LINES: usize = 20;
    const TITLE_LEN: usize = 80;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    for (i, line) in reader.lines().enumerate() {
        if i >= MAX_LINES {
            break;
        }
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let raw: RawLine = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if raw.line_type != Some("user") {
            continue;
        }
        let message = match raw.message {
            Some(m) => m,
            None => continue,
        };
        if message.role != Some("user") {
            continue;
        }

        let content_text = extract_text(&message.content);
        let title = truncate(&content_text, TITLE_LEN);

        let timestamp = raw
            .timestamp
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        return Ok(Some(SessionHeader {
            title,
            cwd: raw.cwd.map(PathBuf::from),
            git_branch: raw.git_branch,
            first_timestamp: timestamp,
        }));
    }

    Ok(None)
}

fn extract_text(content: &Option<serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sample-claude.jsonl")
    }

    #[test]
    fn reads_header_from_fixture() {
        let header = read_session_header(&fixture_path())
            .expect("read ok")
            .expect("some header");
        assert_eq!(header.title, "Help me refactor the auth module");
        assert_eq!(
            header.cwd,
            Some(PathBuf::from("/Users/kgd/IdeaProjects/aieye"))
        );
        assert_eq!(header.git_branch, Some("main".to_string()));
        assert!(header.first_timestamp.is_some());
    }

    #[test]
    fn truncates_long_title() {
        let s = "a".repeat(200);
        assert!(truncate(&s, 80).chars().count() <= 81); // +1 for "…"
    }
}
```

- [ ] **Step 3: parser/mod.rs 에 export 추가**

Edit `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/parser/mod.rs`:

```rust
pub mod project_slug;
pub mod claude_jsonl;

pub use project_slug::decode_project_slug;
pub use claude_jsonl::{read_session_header, SessionHeader};
```

- [ ] **Step 4: 테스트 실행**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye/src-tauri
cargo test --lib claude_jsonl 2>&1 | tail -15
```

Expected: 2/2 통과.

- [ ] **Step 5: Commit**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
git add src-tauri/src/parser/ src-tauri/tests/fixtures/
git commit -m "feat(parser): Claude JSONL 헤더 파서 (title/cwd/gitBranch 추출)"
```

---

## Task 8: Rust — SessionAdapter 트레이트

**Files:**
- Create: `src-tauri/src/sessions/adapter.rs`
- Modify: `src-tauri/src/sessions/mod.rs`

- [ ] **Step 1: 트레이트 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/adapter.rs`:

```rust
use super::model::{CliKind, Session};
use async_trait::async_trait;
use std::path::PathBuf;

/// CLI 종류별로 JSONL 세션을 스캔·파싱하는 어댑터.
#[async_trait]
pub trait SessionAdapter: Send + Sync {
    fn cli(&self) -> CliKind;

    /// 이 adapter 가 감시할 파일시스템 경로들.
    fn watch_paths(&self) -> Vec<PathBuf>;

    /// 현재 시점에 존재하는 모든 세션 목록 반환.
    async fn scan(&self) -> anyhow::Result<Vec<Session>>;
}
```

- [ ] **Step 2: Cargo.toml 에 async-trait 추가**

Edit `src-tauri/Cargo.toml` `[dependencies]`:

```toml
async-trait = "0.1"
```

- [ ] **Step 3: mod.rs 에 export**

Edit `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/mod.rs`:

```rust
pub mod model;
pub mod adapter;

pub use model::{CliKind, Session, SessionState};
pub use adapter::SessionAdapter;
```

- [ ] **Step 4: 빌드 확인**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye/src-tauri
cargo build 2>&1 | tail -3
```

Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
git add src-tauri/Cargo.toml src-tauri/src/sessions/
git commit -m "feat(sessions): SessionAdapter async trait"
```

---

## Task 9: Rust — ClaudeAdapter.scan() 구현

**Files:**
- Create: `src-tauri/src/sessions/claude.rs`
- Modify: `src-tauri/src/sessions/mod.rs`

- [ ] **Step 1: ClaudeAdapter 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/claude.rs`:

```rust
use super::adapter::SessionAdapter;
use super::model::{CliKind, Session, SessionState};
use crate::parser::{claude_jsonl::read_session_header, decode_project_slug};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// `~/.claude/projects/` 기반 Claude Code 세션 어댑터.
pub struct ClaudeAdapter {
    root: PathBuf,
    recent_threshold_minutes: u32,
}

impl ClaudeAdapter {
    pub fn with_defaults() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Self {
            root: PathBuf::from(home).join(".claude/projects"),
            recent_threshold_minutes: 60,
        }
    }

    pub fn new(root: PathBuf, recent_threshold_minutes: u32) -> Self {
        Self {
            root,
            recent_threshold_minutes,
        }
    }

    fn classify(&self, mtime: DateTime<Utc>) -> SessionState {
        let elapsed = Utc::now() - mtime;
        if elapsed.num_minutes() <= self.recent_threshold_minutes as i64 {
            SessionState::Recent
        } else {
            SessionState::Stale
        }
        // Running 은 Plan 2 이후 ProcessDetector 가 덮어씀.
    }

    fn session_from_file(&self, jsonl: &Path, project_slug: &str) -> Option<Session> {
        let id = jsonl.file_stem()?.to_string_lossy().to_string();

        let metadata = fs::metadata(jsonl).ok()?;
        let mtime: DateTime<Utc> = system_to_utc(metadata.modified().ok()?);

        let header = read_session_header(jsonl).ok().flatten();
        let title = header
            .as_ref()
            .map(|h| h.title.clone())
            .unwrap_or_else(|| "(untitled)".to_string());

        let project_path = header
            .as_ref()
            .and_then(|h| h.cwd.clone())
            .or_else(|| decode_project_slug(project_slug));

        let git_branch = header.as_ref().and_then(|h| h.git_branch.clone());

        Some(Session {
            id,
            cli: CliKind::Claude,
            title,
            project_path,
            git_branch,
            jsonl_path: jsonl.to_path_buf(),
            last_activity: mtime,
            message_count: None,
            state: self.classify(mtime),
        })
    }
}

fn system_to_utc(t: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(t)
}

#[async_trait]
impl SessionAdapter for ClaudeAdapter {
    fn cli(&self) -> CliKind {
        CliKind::Claude
    }

    fn watch_paths(&self) -> Vec<PathBuf> {
        vec![self.root.clone()]
    }

    async fn scan(&self) -> anyhow::Result<Vec<Session>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }

        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().to_string();
            let dir = entry.path();

            for f in fs::read_dir(&dir)? {
                let f = f?;
                let path = f.path();
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    if let Some(s) = self.session_from_file(&path, &slug) {
                        sessions.push(s);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn scans_fixture_directory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        // fake project/session
        let proj_dir = root.join("-Users-kgd-IdeaProjects-aieye");
        fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join("abc-123.jsonl");
        let mut f = File::create(&jsonl).unwrap();
        writeln!(
            f,
            r#"{{"type":"user","message":{{"role":"user","content":"Test prompt"}},"cwd":"/Users/kgd/IdeaProjects/aieye","sessionId":"abc-123","gitBranch":"main","timestamp":"2026-04-18T12:00:00.000Z"}}"#
        )
        .unwrap();

        let adapter = ClaudeAdapter::new(root.to_path_buf(), 60);

        let sessions = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(adapter.scan())
            .expect("scan ok");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "abc-123");
        assert_eq!(sessions[0].title, "Test prompt");
        assert_eq!(sessions[0].cli, CliKind::Claude);
    }
}
```

- [ ] **Step 2: Cargo.toml dev-dependencies 에 tempfile 추가**

Edit `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: mod.rs 에 export**

Edit `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/sessions/mod.rs`:

```rust
pub mod model;
pub mod adapter;
pub mod claude;

pub use model::{CliKind, Session, SessionState};
pub use adapter::SessionAdapter;
pub use claude::ClaudeAdapter;
```

- [ ] **Step 4: 테스트 실행**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye/src-tauri
cargo test --lib claude::tests 2>&1 | tail -15
```

Expected: 1/1 통과.

- [ ] **Step 5: Commit**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
git add src-tauri/Cargo.toml src-tauri/src/sessions/
git commit -m "feat(sessions): ClaudeAdapter.scan — ~/.claude/projects JSONL 스캔"
```

---

## Task 10: Tauri `list_sessions` command

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: commands.rs 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/commands.rs`:

```rust
use crate::sessions::{ClaudeAdapter, Session, SessionAdapter};

#[tauri::command]
pub async fn list_sessions() -> Result<Vec<Session>, String> {
    let adapter = ClaudeAdapter::with_defaults();
    adapter
        .scan()
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: main.rs 에 command 등록**

Edit `/Users/gideok-kwon/IdeaProjects/aieye/src-tauri/src/main.rs` — Builder 체인에 `.invoke_handler(...)` 추가:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod sessions;
mod parser;
mod commands;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aieye=info".into()),
        )
        .init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_sessions])
        .setup(|app| {
            tray::build_tray(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: 빌드 확인**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | tail -3
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat(rust): list_sessions Tauri command"
```

---

## Task 11: TypeScript — Session 타입 + IPC wrapper

**Files:**
- Create: `src/types/session.ts`
- Create: `src/ipc/tauri.ts`

- [ ] **Step 1: 타입 정의**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/types/session.ts`:

```ts
export type CliKind = "claude" | "codex";

export type SessionState = "running" | "recent" | "stale";

export interface Session {
  id: string;
  cli: CliKind;
  title: string;
  project_path: string | null;
  git_branch: string | null;
  jsonl_path: string;
  last_activity: string; // RFC3339
  message_count: number | null;
  state: SessionState;
}
```

- [ ] **Step 2: invoke 래퍼**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/ipc/tauri.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { Session } from "../types/session";

export async function listSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_sessions");
}
```

- [ ] **Step 3: Commit**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
git add src/types/ src/ipc/
git commit -m "feat(ipc): Session 타입 + listSessions invoke wrapper"
```

---

## Task 12: React — useSessions 훅 + SessionList 렌더

**Files:**
- Create: `src/hooks/useSessions.ts`
- Create: `src/components/SessionList.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: useSessions 훅**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/hooks/useSessions.ts`:

```ts
import { useEffect, useState } from "react";
import { listSessions } from "../ipc/tauri";
import type { Session } from "../types/session";

export function useSessions() {
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    listSessions()
      .then((s) => {
        if (!cancelled) setSessions(s);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return { sessions, error };
}
```

- [ ] **Step 2: SessionList 컴포넌트**

Create `/Users/gideok-kwon/IdeaProjects/aieye/src/components/SessionList.tsx`:

```tsx
import type { Session } from "../types/session";

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

interface Props {
  sessions: Session[];
}

export function SessionList({ sessions }: Props) {
  if (sessions.length === 0) {
    return <div className="empty">No sessions yet.</div>;
  }

  return (
    <div className="session-list">
      {sessions.map((s) => (
        <div key={`${s.cli}-${s.id}`} className="session-row">
          <span className="state">{stateDot(s.state)}</span>
          <span className="cli">[{s.cli}]</span>
          <div className="body">
            <div className="title">{s.title}</div>
            <div className="sub">
              {s.project_path ?? "unknown path"}
              {s.git_branch && <> · {s.git_branch}</>}
              <> · {relativeTime(s.last_activity)}</>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: styles.css 에 세션 스타일 추가**

Append to `/Users/gideok-kwon/IdeaProjects/aieye/src/styles.css`:

```css
.session-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.session-row {
  display: flex;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  align-items: flex-start;
}

.session-row:last-child { border-bottom: none; }

.session-row .state { flex: 0 0 auto; font-size: 10px; padding-top: 2px; }
.session-row .cli {
  flex: 0 0 auto;
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.06);
  color: #555;
}
.session-row .body { flex: 1; min-width: 0; }
.session-row .title {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.session-row .sub {
  font-size: 11px;
  color: #888;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.error {
  padding: 16px;
  color: #c00;
  font-size: 12px;
}
```

- [ ] **Step 4: App.tsx 에서 렌더**

Overwrite `/Users/gideok-kwon/IdeaProjects/aieye/src/App.tsx`:

```tsx
import "./styles.css";
import { useSessions } from "./hooks/useSessions";
import { SessionList } from "./components/SessionList";

export default function App() {
  const { sessions, error } = useSessions();

  return (
    <div className="app">
      <div className="header">
        👁 aieye {sessions && <span style={{ float: "right", fontWeight: 400, color: "#888" }}>{sessions.length}</span>}
      </div>
      {error && <div className="error">{error}</div>}
      {sessions === null && !error && <div className="empty">Scanning…</div>}
      {sessions && <SessionList sessions={sessions} />}
    </div>
  );
}
```

- [ ] **Step 5: 빌드 + 실행**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
pnpm tauri build --debug 2>&1 | tail -3
pkill -f "aieye.app/Contents/MacOS/aieye" 2>/dev/null
open src-tauri/target/debug/bundle/macos/aieye.app
sleep 3
pgrep -f aieye.app/Contents/MacOS/aieye
```

메뉴바 클릭 → 실제 `~/.claude/projects` 세션 리스트가 떠야 함. 실제 로컬 프로젝트들 (msa, aieye, muxbar 등) 이 표시됨.

```bash
pkill -f "aieye.app/Contents/MacOS/aieye"
```

- [ ] **Step 6: Commit**

```bash
git add src/
git commit -m "feat(ui): useSessions 훅 + SessionList 컴포넌트 — 실제 데이터 렌더"
```

---

## Task 13: README + ADR-0001

**Files:**
- Modify: `README.md`
- Create: `docs/adr/ADR-0001-tauri-v2-react.md`
- Create: `docs/README.md`

- [ ] **Step 1: ADR-0001 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/docs/adr/ADR-0001-tauri-v2-react.md`:

```markdown
# ADR-0001: Tauri v2 + React 채택 (Swift / Electron 대비)

- Status: Accepted
- Date: 2026-04-18

## Context

AI CLI 세션 뷰어의 UI 요구:
- macOS 메뉴바 상주
- 채팅 메시지 렌더링 (마크다운 + 코드 하이라이트 + 미래: diff/이미지)
- 로컬 JSONL 파일 감시 + 파싱
- 가능하면 Linux 도 지원

## Decision

**Tauri v2 (Rust 백엔드) + React + TypeScript + Vite**.

## Alternatives considered

### Swift / SwiftUI (muxbar 동일 스택)
- ✅ 네이티브 UX 최고, muxbar 경험 재사용
- ❌ 채팅 렌더링 (markdown/code highlight) 구현 비용 큼
- ❌ Linux 불가

### Electron
- ✅ 가장 풍부한 UI 생태계
- ❌ 번들 100MB+, 메모리 250MB+ — 메뉴바 상주 앱에 부적합

### Go + systray / Fyne
- ✅ 단일 바이너리
- ❌ 채팅 렌더링 UI 난제, 메뉴바 제약

## Consequences

**장점**:
- HTML + React 로 채팅 UI 자유롭게 (react-markdown + shiki 등 생태계)
- WKWebView 재사용 → 번들 ~15MB, 메모리 ~100MB (Electron 대비 1/3)
- Linux/Windows 가능성 "공짜" (세부 대응은 별도 작업)
- Rust 백엔드 = 성능 확실 + 경험 획득

**단점**:
- 2-stack (Rust + TS)
- macOS 네이티브 특수 동작 일부는 Swift 만큼 정교하지 않을 수 있음

## References

- [Tauri v2 docs](https://tauri.app)
- [pot-desktop](https://github.com/pot-app/pot-desktop) — Tauri 메뉴바 앱 예시
```

- [ ] **Step 2: docs/README.md 작성**

Create `/Users/gideok-kwon/IdeaProjects/aieye/docs/README.md`:

```markdown
# aieye Docs

## Specs
- [v0.1 Design](specs/2026-04-18-v0.1-design.md)

## Plans
- [Plan 1 — Skeleton + ClaudeAdapter](plans/2026-04-18-plan-1-skeleton-claude-adapter.md)
- Plan 2 — UI polish + CodexAdapter *(예정)*
- Plan 3 — Live preview + FS watcher *(예정)*
- Plan 4 — Distribution *(예정)*

## ADRs
- [ADR-0001: Tauri v2 + React 채택](adr/ADR-0001-tauri-v2-react.md)
```

- [ ] **Step 3: README.md 에 현재 상태 반영**

Edit `/Users/gideok-kwon/IdeaProjects/aieye/README.md` — "> **Status**: ..." 라인을 다음으로 교체:

```markdown
> **Status**: Plan 1 complete — Tauri skeleton + ClaudeAdapter. Lists real Claude Code sessions from `~/.claude/projects`.
```

그리고 파일 아래쪽 `## Documentation` 섹션을 다음으로 확장:

```markdown
## Documentation

- [v0.1 Design spec](docs/specs/2026-04-18-v0.1-design.md)
- [Implementation plans](docs/README.md)
- [Architecture decisions (ADRs)](docs/adr)

## Development

Requires Rust 1.70+, Node 18+, pnpm 8+.

```bash
pnpm install
pnpm tauri dev        # dev mode with HMR
pnpm tauri build      # production .app bundle
```
```

- [ ] **Step 4: Commit + tag**

```bash
cd /Users/gideok-kwon/IdeaProjects/aieye
git add docs/ README.md
git commit -m "docs: ADR-0001 + README/docs 인덱스 + Plan 1 완료 표시"
git tag -a plan-1-complete -m "Plan 1: Skeleton + ClaudeAdapter 완료"
git push origin main --follow-tags
```

---

## Plan 1 완료 기준 (Acceptance Criteria)

- [x] `pnpm install` + `pnpm tauri build --debug` 성공
- [x] 실행 시 메뉴바에 아이콘 출현, Dock 아이콘 없음 (LSUIElement)
- [x] 트레이 클릭 → panel window 토글
- [x] panel 안에 실제 `~/.claude/projects` 세션 리스트 표시
- [x] 각 row: 상태 dot + CLI badge + title + project path + branch + relative time
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` 통과
- [x] ADR-0001 + 문서 인덱스 작성
- [x] `plan-1-complete` 태그 + push

## Plan 2 예고

- CodexAdapter (`~/.codex/sessions/` 스캔)
- Row 클릭 → resume (Terminal launcher)
- Settings 서브메뉴 (recent threshold, preferred terminal)
- Quit 메뉴 + ⋯ 행별 액션
