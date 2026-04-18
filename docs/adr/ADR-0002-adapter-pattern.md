# ADR-0002: SessionAdapter 트레이트 기반 CLI 확장성

- Status: Accepted
- Date: 2026-04-18

## Context

Claude Code / Codex / 향후 Cursor / Aider / Gemini 등 여러 AI CLI 를
통합 리스트로 표시해야 함. CLI 마다 세션 저장 경로와 JSONL 스키마가
모두 다름.

## Decision

- `SessionAdapter` async trait 정의 (scan / watch_paths / cli)
- CLI 별 구현: `ClaudeAdapter`, `CodexAdapter`
- `SessionCoordinator` 가 여러 adapter 를 병합해 단일 `Vec<Session>` 반환
- 새 CLI 추가 = 1 파일 + 1 test 케이스 + coordinator 에 1 줄

## Consequences

**장점**
- 확장 용이. Cursor/Aider 등 추가 시 기존 코드 영향 최소
- 각 adapter 단위 테스트 가능 (tempdir + fixture)
- Coordinator 에서 중앙 집중 정렬/dedupe

**단점**
- `Box<dyn SessionAdapter>` 동적 디스패치 (성능 영향 미미)
- adapter 내부에 JSONL 포맷 분기가 숨겨짐 → 포맷 버전 다양성이 커지면 재설계 필요

## Notes

Codex JSONL 스키마는 `response_item` + `payload.role: user` + `payload.content[].text`
형태이며, `<environment_context>` 등 auto-generated 메시지가 첫 줄에
등장하므로 필터 필수. Claude Code 는 `type: user` + `message.content`
구조로 단순. 포맷 차이를 adapter 레벨에서 흡수.
