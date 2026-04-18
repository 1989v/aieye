#!/bin/bash
# aieye .app bundle build script — Xcode 불필요 (Command Line Tools + Rust + pnpm)
#
# Usage:
#   ./build.sh              # tauri build --debug + LSUIElement 주입 + ad-hoc sign
#   ./build.sh release      # production build + ad-hoc sign
#   ./build.sh open         # 위 + 즉시 실행
#   ./build.sh install      # 위 + /Applications 복사

set -euo pipefail

MODE="${1:-debug}"
REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"

# rustup via brew 는 기본 PATH 에 없음 — 자동 추가
if ! command -v cargo >/dev/null 2>&1; then
    export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fi

echo "[1/4] Dependencies check"
for tool in pnpm cargo rustc; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "  ✗ $tool 미설치. README 'Requirements' 섹션 참고"
        exit 1
    fi
done
echo "  ✓ pnpm $(pnpm --version)"
echo "  ✓ cargo $(cargo --version | awk '{print $2}')"

echo "[2/4] pnpm install (필요시)"
cd "$REPO_ROOT"
pnpm install 2>&1 | tail -3

echo "[3/4] pnpm tauri build ($MODE)"
case "$MODE" in
    debug|open|install)
        pnpm tauri build --debug 2>&1 | tail -8
        APP_DIR="$REPO_ROOT/src-tauri/target/debug/bundle/macos/aieye.app"
        ;;
    release)
        pnpm tauri build 2>&1 | tail -8
        APP_DIR="$REPO_ROOT/src-tauri/target/release/bundle/macos/aieye.app"
        ;;
    *)
        echo "unknown mode: $MODE"
        exit 1
        ;;
esac

if [ ! -d "$APP_DIR" ]; then
    echo "✗ build failed — $APP_DIR not found"
    exit 1
fi

echo "[4/4] LSUIElement 주입 + ad-hoc codesign"
plutil -insert LSUIElement -bool true "$APP_DIR/Contents/Info.plist" 2>/dev/null || \
    plutil -replace LSUIElement -bool true "$APP_DIR/Contents/Info.plist"
codesign --deep --force --sign - "$APP_DIR"
xattr -dr com.apple.quarantine "$APP_DIR" 2>/dev/null || true
echo "  ✓ $APP_DIR"

case "$MODE" in
    open)
        pkill -f "aieye.app/Contents/MacOS/aieye" 2>/dev/null || true
        sleep 1
        open "$APP_DIR"
        echo "  → launched"
        ;;
    install)
        rm -rf /Applications/aieye.app
        cp -R "$APP_DIR" /Applications/aieye.app
        echo "  → /Applications/aieye.app"
        ;;
esac
