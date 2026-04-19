# aieye Distribution

## 릴리스 절차

1. 버전 확정 (semver) → `./Release.sh 0.1.0`
2. `dist/aieye-0.1.0.dmg` 생성됨
3. GitHub Release 생성 → `.dmg` 업로드
   ```bash
   gh release create v0.1.0 dist/aieye-0.1.0.dmg \
       --title "aieye 0.1.0" \
       --notes "..."
   ```
4. SHA256 복사 → `HomebrewTap/aieye.rb` 의 `sha256` 업데이트
5. `1989v/homebrew-tap` 별도 레포에 `Casks/aieye.rb` 푸시
6. 사용자 검증: `brew install --cask 1989v/tap/aieye`

## 전제 조건

- `brew install create-dmg` (dmg 패키징)
- Rust + Node + pnpm (빌드 도구)
- Tauri CLI (`pnpm tauri`)

## Ad-hoc 서명의 한계

Apple Developer 계정 없음 → notarize 불가. 사용자는 첫 실행 시 우클릭 → 열기 필요.
Homebrew cask 가 `xattr -dr com.apple.quarantine` 로 자동 우회.

## 재사용되는 검증 커맨드

```bash
# 번들 크기
du -sh dist/aieye-*.dmg

# 서명 확인
codesign -dv --verbose=4 dist/aieye-*.app 2>&1 | head

# SHA256
shasum -a 256 dist/aieye-*.dmg
```
