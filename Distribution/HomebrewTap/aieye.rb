cask "aieye" do
  version "0.1.0"
  sha256 "TBD_AFTER_BUILD"

  url "https://github.com/1989v/aieye/releases/download/v#{version}/aieye-#{version}.dmg"
  name "aieye"
  desc "Menu bar app for monitoring AI CLI sessions (Claude Code, Codex)"
  homepage "https://github.com/1989v/aieye"

  depends_on macos: ">= :ventura"

  app "aieye.app"

  postflight do
    system_command "xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/aieye.app"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/com.1989v.aieye",
    "~/Library/Preferences/com.1989v.aieye.plist",
    "~/Library/Caches/com.1989v.aieye",
    "~/Library/WebKit/com.1989v.aieye",
  ]
end
