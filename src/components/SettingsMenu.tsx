import { useEffect, useState } from "react";
import type { TerminalApp } from "../types/session";
import { useSettings } from "../hooks/useSettings";
import { listInstalledTerminals } from "../ipc/tauri";

const LABELS: Record<TerminalApp, string> = {
  terminal: "Terminal",
  iterm2: "iTerm2",
  alacritty: "Alacritty",
  kitty: "kitty",
};

export function SettingsMenu() {
  const { settings, update } = useSettings();
  const [open, setOpen] = useState(false);
  const [installed, setInstalled] = useState<TerminalApp[] | null>(null);

  useEffect(() => {
    listInstalledTerminals().then(setInstalled).catch(() => setInstalled([]));
  }, []);

  if (!settings) return null;

  const options = installed ?? [];

  return (
    <div className="settings-menu">
      <button className="settings-toggle" onClick={() => setOpen((o) => !o)}>
        ⚙ Settings
      </button>
      {open && (
        <div className="settings-panel">
          <label>
            <span>Preferred terminal</span>
            <select
              value={settings.preferred_terminal}
              onChange={(e) => update({ preferred_terminal: e.target.value as TerminalApp })}
            >
              {options.length === 0 && (
                <option value={settings.preferred_terminal}>
                  {LABELS[settings.preferred_terminal]}
                </option>
              )}
              {options.map((t) => (
                <option key={t} value={t}>
                  {LABELS[t]}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Recent threshold (min)</span>
            <input
              type="number"
              min={1}
              max={1440}
              value={settings.recent_threshold_minutes}
              onChange={(e) =>
                update({ recent_threshold_minutes: Number(e.target.value) || 60 })
              }
            />
          </label>
        </div>
      )}
    </div>
  );
}
