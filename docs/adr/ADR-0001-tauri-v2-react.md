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
- Tauri v2 의 `bundle.macOS.infoPlist` 필드가 실제로 merge 안 되는
  이슈 확인됨 — build.sh 에서 `plutil` 로 LSUIElement 사후 주입으로
  우회

## References

- [Tauri v2 docs](https://tauri.app)
- [pot-desktop](https://github.com/pot-app/pot-desktop) — Tauri 메뉴바 앱 예시
- [muxbar](https://github.com/1989v/muxbar) — 동일 저자 Swift 버전
