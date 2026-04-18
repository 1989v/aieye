# aieye

**Language:** [English](README.md) | [한국어](README.ko.md)

> Menu bar app for AI CLI sessions — discover, resume, and preview active conversations at a glance.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![macOS 13+](https://img.shields.io/badge/macOS-13.0+-blue.svg)](https://www.apple.com/macos/)
[![Tauri v2](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61DAFB.svg)](https://react.dev)

> **Status**: In design (v0.1 spec complete). Implementation starts next.

## What it solves

If you use AI CLIs (Claude Code, Codex, …) across many projects, you know the pain: `/resume` asks you to scroll through dozens of sessions, and you can never remember which project the last one was in.

**aieye** lives in your menu bar and shows every session across every project, sorted by activity, with a live preview on hover and one-click resume.

## Features (v0.1 MVP)

- Unified list of sessions from Claude Code + Codex
- Per-session metadata: project path, git branch, first message (as title), last activity
- Three state badges: 🟢 running · 🟡 recent · 🔘 stale
- **Click a row → resume that conversation** in your preferred terminal
- **Hover a row → live preview** (markdown + code highlight) of the last messages
- Filesystem watcher → menu updates in real time (no polling)
- Adapter architecture — adding new AI CLIs is a single trait implementation

## Documentation

- [v0.1 Design spec](docs/specs/2026-04-18-v0.1-design.md)

## License

MIT © 2026 kgd
