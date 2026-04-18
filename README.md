# aieye

> Menu bar app for AI CLI sessions — discover, resume, and preview active conversations at a glance.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![macOS 13+](https://img.shields.io/badge/macOS-13.0+-blue.svg)](https://www.apple.com/macos/)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB.svg)](https://react.dev)

> **Status**: Plan 2 complete — Claude + Codex sessions, click-to-resume in preferred terminal, row actions, settings persistence.

## What it solves

If you use AI CLIs (Claude Code, Codex, …) across many projects, you know the pain: `/resume` asks you to scroll through dozens of sessions, and you can never remember which project the last one was in.

**aieye** lives in your menu bar and shows every session across every project, sorted by activity, with a live preview on hover and one-click resume.

## Features (v0.1 MVP — in progress)

- ✅ Unified list of sessions from Claude Code (Codex next)
- ✅ Per-session metadata: project path, git branch, first message (as title), last activity
- ✅ Three state badges: 🟢 running · 🟡 recent · 🔘 stale (Running detection pending)
- 🔜 **Click a row → resume** the conversation in your preferred terminal
- 🔜 **Hover a row → live preview** (markdown + code highlight) of the last messages
- 🔜 Filesystem watcher → menu updates in real time (no polling)
- ✅ Adapter architecture — adding new AI CLIs is a single trait implementation

## Requirements

- macOS 13 (Ventura) or later
- Node 18+ and pnpm 8+ (`brew install pnpm`)
- Rust 1.70+ (`brew install rustup && rustup default stable`)
- Xcode Command Line Tools (`xcode-select --install`)

## Quick start

```bash
git clone https://github.com/1989v/aieye.git
cd aieye
./build.sh open
```

`build.sh`:
1. `pnpm install`
2. `pnpm tauri build --debug` → `aieye.app` bundle
3. `plutil -insert LSUIElement -bool true` (menu-bar mode, no Dock icon)
4. Ad-hoc codesign
5. Launch the app

## Development

```bash
pnpm install
pnpm tauri dev        # HMR dev mode (opens a dev window)

# or tests only
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"  # brew-rustup shim
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

## Documentation

- [v0.1 Design spec](docs/specs/2026-04-18-v0.1-design.md)
- [Implementation plans](docs/README.md)
- [Architecture decisions (ADRs)](docs/adr)

## License

MIT © 2026 kgd
