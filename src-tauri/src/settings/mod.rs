pub mod model;

pub use model::Settings;

use std::path::PathBuf;

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join("Library/Application Support/com.1989v.aieye/settings.json")
}

pub fn load() -> Settings {
    let path = settings_path();
    if !path.exists() {
        return Settings::default();
    }
    // 과거 버전에서 preferred_terminal 이 "warp" 등 제거된 값일 수 있음 → 무효시 default
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, json)?;
    Ok(())
}
