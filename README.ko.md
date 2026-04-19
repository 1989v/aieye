# aieye

**Language:** [English](README.md) | [한국어](README.ko.md)

> AI CLI 세션(Claude Code, Codex) 을 메뉴바에서 한눈에. 어느 프로젝트에서 어떤 대화를 하다 말았는지 잊어도 됨. 클릭 한 번에 resume.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![macOS 13+](https://img.shields.io/badge/macOS-13.0+-blue.svg)](https://www.apple.com/macos/)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://tauri.app)
[![React 19](https://img.shields.io/badge/React-19-61DAFB.svg)](https://react.dev)

## 주요 기능

- **통합 세션 리스트** — 모든 프로젝트의 Claude Code / Codex 세션을 활동 시간순으로 한 줄씩. `/resume` 에서 스크롤 할 필요 없음.
- **스마트 resume** — 행 클릭 → 실행 중이면 해당 터미널 탭을 포커스 (Terminal/iTerm2) 또는 호스트 IDE 활성화 (VS Code/Cursor/JetBrains). 실행 중 아니면 선호 터미널에서 `claude --resume <id>` / `codex resume <id>` 로 새로 런칭.
- **실시간 활성 상태 감지** — 세션별 `generating…` / `idle` 뱃지. Claude 는 `stop_reason` 기반, Codex 는 `task_started/complete` 이벤트 기반 휴리스틱.
- **애니메이션 트레이 아이콘** — 눈 테마. 아무도 작업 안 할 땐 감은 눈, 하나라도 생성 중이면 깜빡임, 최근에 완료된 세션 있으면 뜬 눈 + 숫자. 트레이 클릭 / 행 클릭 / 24시간 경과 시 자동 해제.
- **세션별 ✓ 체크마크** — 방금 응답 완료된 세션에 플래그. 행 클릭 / 다른 터미널에서 유저가 새 메시지 보내면 자동 해제.
- **인라인 + hover 미리보기** — 각 행에 마지막 user/assistant 텍스트 요약 1줄씩. hover 시 우측 패널에 최근 10턴 전체 텍스트.
- **정리 모드 + 일괄 휴지통 이동** — CLI / 연령 / 검색 필터, 체크박스 다중 선택, 오래된 세션 일괄 휴지통. **7일 안전 윈도우** 로 최근 세션은 프론트+백엔드 이중 차단.
- **행별 Overflow 메뉴** — Reveal in Finder · Open in new terminal · Copy session ID · Move to trash.
- **어댑터 구조** — `SessionAdapter` trait 구현 하나로 새 AI CLI 지원 추가.

## 메뉴바 아이콘 상태

| 아이콘 | 상태 |
|---|---|
| 👁️ (감은 눈) | 활성 세션 없음 |
| 👁️ (깜빡임) + N | N개 세션이 응답 생성 중 |
| 👁️ (뜬 눈) + N | N개 세션이 방금 응답 완료, 확인 대기 |

## 요구 사항

- macOS 13 (Ventura) 이상
- Node 18+ / pnpm 8+ (`brew install pnpm`)
- Rust 1.70+ (`brew install rustup && rustup default stable`)
- Xcode Command Line Tools (`xcode-select --install`)

## Quick start

```bash
git clone https://github.com/1989v/aieye.git
cd aieye
./build.sh install
open /Applications/aieye.app
```

메뉴바에 파란 눈 아이콘이 뜨면 성공. 클릭해서 세션 리스트 확인.

## 설치 옵션

### 1. 소스에서 빌드 (현재 기본)

```bash
git clone https://github.com/1989v/aieye.git
cd aieye

./build.sh            # Debug 빌드 + .app 번들
./build.sh release    # Production 빌드
./build.sh open       # 빌드 + 즉시 실행
./build.sh install    # 빌드 + /Applications 복사
```

`build.sh` 동작:
1. `pnpm` / `cargo` / `rustc` 존재 확인
2. `pnpm install` + `pnpm tauri build`
3. `LSUIElement=true` 주입 (메뉴바 모드, Dock 아이콘 없음)
4. `codesign --sign -` 로 ad-hoc 서명 (Apple Developer 계정 불필요)
5. Quarantine 속성 제거 → Gatekeeper 경고 없이 실행

### 2. Homebrew cask *(아직 배포 전)*

```bash
brew install --cask 1989v/tap/aieye
```

### 3. `.dmg` 사전 빌드 *(아직 배포 전)*

각 [GitHub Release](https://github.com/1989v/aieye/releases) 에 첨부. 첫 실행 시 우클릭 → 열기 (ad-hoc 서명이라 공증 안 됨).

## 개발

```bash
pnpm install
pnpm tauri dev        # HMR 개발 모드

# 테스트
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## 지원 CLI

| CLI | 탐지 | Resume | 라이브 미리보기 | 활성 상태 |
|---|---|---|---|---|
| Claude Code | ✅ | ✅ `--resume <id>` | ✅ | ✅ `stop_reason` 기반 |
| Codex | ✅ | ✅ `resume <id>` | ✅ | ✅ `task_started/complete` 이벤트 |
| Gemini CLI / Aider / GPT CLI 등 | 🔜 | 🔜 | 🔜 | 🔜 |

CLI 추가 = [`SessionAdapter`](src-tauri/src/sessions/adapter.rs) 구현 + coordinator 등록.

## 키 / 인터랙션

| 액션 | 동작 |
|---|---|
| 트레이 아이콘 클릭 | 패널 토글 · 모든 finished 확인 처리 |
| 행 클릭 | 스마트 resume (기존 터미널 포커스 또는 새로 런칭) |
| 정리 모드에서 행 클릭 | 선택 체크박스 토글 |
| 행 hover | 우측 패널에 최근 대화 턴 로드 |
| ⋯ 메뉴 | 세션별 액션 |
| ESC / 바깥 클릭 | 메뉴 / 다이얼로그 닫기 |

## 디자인 & 문서

- [v0.1 Design spec](docs/specs/2026-04-18-v0.1-design.md)
- [구현 플랜](docs/plans/)
- [Architecture Decision Records](docs/adr/)
- [GitHub 발견성 가이드](docs/guides/github-discoverability.md)

## 라이선스

[MIT](LICENSE) © 2026 kgd
