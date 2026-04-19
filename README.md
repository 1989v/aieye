# aieye

**Language:** [English](README.md) | [한국어](README.ko.md)

> A native macOS menu bar app for monitoring AI CLI sessions (Claude Code, Codex) — never lose a conversation, see what's running at a glance, resume any session in one click.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![macOS 13+](https://img.shields.io/badge/macOS-13.0+-blue.svg)](https://www.apple.com/macos/)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://tauri.app)
[![React 19](https://img.shields.io/badge/React-19-61DAFB.svg)](https://react.dev)

## Features

- **Unified session list** — every Claude Code and Codex session across every project, sorted by activity. One source of truth for `/resume`.
- **Smart resume** — click a row → if the session is running, focus its existing terminal tab (Terminal / iTerm2) or activate the host IDE (VS Code / Cursor / JetBrains). Not running → launch in your preferred terminal with `claude --resume <id>` / `codex resume <id>`.
- **Live activity detection** — per-session `generating…` / `idle` badge based on JSONL tail heuristic (Claude: `stop_reason` aware; Codex: `task_started/complete` event stream).
- **Animated tray icon** — eye-themed frames reflect global state: closed eye idle, blink animation while any session is generating, open eye + count when sessions are waiting for your attention. Auto-clears on tray click / per-session interaction / 24h expiry.
- **Per-session ✓ checkmark** — sessions that just finished are flagged until you interact again (click row, acknowledge, or user types in that terminal).
- **Inline + hover preview** — each row shows last user/assistant text snippet inline (muxbar-style); hovering opens a right panel with the last ~10 turns full text.
- **Cleanup & bulk archive** — filter by CLI / age / search, checkbox multi-select, move old sessions to Trash. **7-day safety window** protects recent sessions in both frontend and backend.
- **Overflow menu per row** — Reveal in Finder · Open in new terminal (force fresh) · Copy session ID · Move to trash (if idle).
- **Adapter architecture** — `SessionAdapter` trait makes adding new AI CLIs a single-module change.

## Menu bar icon states

| Icon | State |
|---|---|
| 👁️ (closed) | No sessions active |
| 👁️ (blink loop) + N | N sessions currently generating responses |
| 👁️ (open) + N | N sessions recently finished, awaiting your attention |

## Requirements

- macOS 13 (Ventura) or later
- Node 18+ and pnpm 8+ (`brew install pnpm`)
- Rust 1.70+ (`brew install rustup && rustup default stable`)
- Xcode Command Line Tools (`xcode-select --install`)

## Quick start

```bash
git clone https://github.com/1989v/aieye.git
cd aieye
./build.sh install
open /Applications/aieye.app
```

An eye icon appears in your menu bar. Click to see every Claude Code and Codex session you've ever run.

## Installation options

### 1. Build from source (current default)

```bash
git clone https://github.com/1989v/aieye.git
cd aieye

./build.sh            # Debug build + .app bundle (keeps fast rebuild cycle)
./build.sh release    # Production build
./build.sh open       # Build + launch from repo
./build.sh install    # Build + copy to /Applications
```

What `build.sh` does:
1. Verifies `pnpm` / `cargo` / `rustc`
2. `pnpm install` + `pnpm tauri build`
3. Injects `LSUIElement=true` into `Info.plist` (menu-bar mode — no Dock icon)
4. Ad-hoc codesigns with `codesign --sign -` (no Apple Developer account needed)
5. Strips the quarantine attribute so the app launches without Gatekeeper prompts

### 2. Homebrew cask *(not yet published)*

```bash
brew install --cask 1989v/tap/aieye
```

### 3. Pre-built `.dmg` *(not yet published)*

Attached to [GitHub Releases](https://github.com/1989v/aieye/releases). On first launch, right-click → Open (ad-hoc signed, not notarized).

## Development

```bash
pnpm install
pnpm tauri dev        # HMR dev mode

# Tests
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## Supported CLIs

| CLI | Detection | Resume | Live preview | Activity |
|---|---|---|---|---|
| Claude Code | ✅ | ✅ `--resume <id>` | ✅ | ✅ `stop_reason` aware |
| Codex | ✅ | ✅ `resume <id>` | ✅ | ✅ `task_started` / `task_complete` events |
| Gemini CLI / Aider / GPT CLI | 🔜 | 🔜 | 🔜 | 🔜 |

Adding a CLI = implementing [`SessionAdapter`](src-tauri/src/sessions/adapter.rs) and registering in the coordinator.

## Keyboard & interaction

| Action | Behavior |
|---|---|
| Click tray icon | Toggle panel · clears all finished acknowledgements |
| Click row | Smart resume (focus existing terminal tab or launch new) |
| Click row in "정리" mode | Toggle selection checkbox |
| Hover row | Right pane loads recent turns |
| ⋯ menu | Per-session actions |
| ESC / outside click | Close menu / confirm dialog |

## Design & documentation

- [v0.1 Design spec](docs/specs/2026-04-18-v0.1-design.md)
- [Implementation plans](docs/plans/)
- [Architecture decisions (ADRs)](docs/adr/)
- [GitHub discoverability guide](docs/guides/github-discoverability.md)

## License

[MIT](LICENSE) © 2026 kgd
