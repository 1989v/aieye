import { useState } from "react";
import type { TerminalApp } from "../types/session";
import { useSettings } from "../hooks/useSettings";

const TERMINALS: { value: TerminalApp; label: string }[] = [
  { value: "terminal", label: "Terminal" },
  { value: "iterm2", label: "iTerm2" },
  { value: "warp", label: "Warp" },
  { value: "alacritty", label: "Alacritty" },
  { value: "kitty", label: "kitty" },
];

export function SettingsMenu() {
  const { settings, update } = useSettings();
  const [open, setOpen] = useState(false);

  if (!settings) return null;

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
              {TERMINALS.map((t) => (
                <option key={t.value} value={t.value}>
                  {t.label}
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
