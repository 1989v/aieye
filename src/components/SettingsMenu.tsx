import { useEffect, useState } from "react";
import type { TerminalApp } from "../types/session";
import type { ReplyMode } from "../types/settings";
import { useSettings } from "../hooks/useSettings";
import { listInstalledTerminals } from "../ipc/tauri";

const LABELS: Record<TerminalApp, string> = {
  terminal: "Terminal",
  iterm2: "iTerm2",
  alacritty: "Alacritty",
  kitty: "kitty",
};

const REPLY_LABELS: Record<ReplyMode, string> = {
  paste: "Paste to terminal",
  headless: "Headless (Claude only, idle)",
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
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={settings.group_by_repo}
              onChange={(e) => update({ group_by_repo: e.target.checked })}
            />
            <span>Group sessions by repo</span>
          </label>
          <fieldset className="settings-fieldset">
            <legend>Reply mode</legend>
            {(Object.keys(REPLY_LABELS) as ReplyMode[]).map((m) => (
              <label key={m} className="settings-radio">
                <input
                  type="radio"
                  name="reply_mode"
                  value={m}
                  checked={settings.reply_mode === m}
                  onChange={() => update({ reply_mode: m })}
                />
                <span>{REPLY_LABELS[m]}</span>
              </label>
            ))}
            <p className="settings-help">
              Paste mode requires Accessibility permission. Open{" "}
              <a
                href="#"
                onClick={(e) => {
                  e.preventDefault();
                  window.open?.(
                    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                  );
                }}
              >
                System Settings → Privacy → Accessibility
              </a>
              .
            </p>
          </fieldset>
        </div>
      )}
    </div>
  );
}
