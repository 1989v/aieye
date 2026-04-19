#!/bin/bash
# aieye 릴리스 빌드 + .dmg 패키징
# 전제: pnpm + Rust + create-dmg (brew install create-dmg)

set -euo pipefail

VERSION="${1:-0.1.0}"
OUT_DIR="dist"
APP_NAME="aieye.app"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cd "$REPO_ROOT"

# rustup via brew
if ! command -v cargo >/dev/null 2>&1; then
    export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
fi

echo "[1/6] Dependencies check"
for tool in pnpm cargo create-dmg; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "  ✗ $tool 미설치"
        [ "$tool" = "create-dmg" ] && echo "    → brew install create-dmg"
        exit 1
    fi
done

echo "[2/6] pnpm install"
pnpm install 2>&1 | tail -3

echo "[3/6] pnpm tauri build (release, universal)"
# Tauri 2 universal build 는 --target universal-apple-darwin 필요
rustup target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true
pnpm tauri build --target universal-apple-darwin 2>&1 | tail -10

APP_DIR="$REPO_ROOT/src-tauri/target/universal-apple-darwin/release/bundle/macos/$APP_NAME"
if [ ! -d "$APP_DIR" ]; then
    # Fallback to native-arch build path
    APP_DIR="$REPO_ROOT/src-tauri/target/release/bundle/macos/$APP_NAME"
fi
if [ ! -d "$APP_DIR" ]; then
    echo "✗ .app bundle not found"
    exit 1
fi

echo "[4/6] LSUIElement + ad-hoc sign"
plutil -insert LSUIElement -bool true "$APP_DIR/Contents/Info.plist" 2>/dev/null || \
    plutil -replace LSUIElement -bool true "$APP_DIR/Contents/Info.plist"
codesign --deep --force --sign - "$APP_DIR"

echo "[5/6] Stage to $OUT_DIR/"
mkdir -p "$OUT_DIR"
STAGE_APP="$OUT_DIR/$APP_NAME"
rm -rf "$STAGE_APP"
cp -R "$APP_DIR" "$STAGE_APP"
xattr -dr com.apple.quarantine "$STAGE_APP" 2>/dev/null || true

echo "[6/6] create-dmg"
DMG="$OUT_DIR/aieye-$VERSION.dmg"
rm -f "$DMG"
create-dmg \
    --volname "aieye $VERSION" \
    --app-drop-link 450 120 \
    "$DMG" \
    "$STAGE_APP"

echo
echo "=== SHA256 ==="
shasum -a 256 "$DMG"
echo
echo "done → $DMG"
