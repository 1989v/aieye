import { useCallback, useEffect, useState } from "react";
import { getSettings, setSettings as saveSettings } from "../ipc/tauri";
import type { Settings } from "../types/settings";

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    getSettings().then(setSettings);
  }, []);

  const update = useCallback((patch: Partial<Settings>) => {
    setSettings((prev) => {
      const base: Settings = prev ?? { preferred_terminal: "terminal", recent_threshold_minutes: 60 };
      const next: Settings = { ...base, ...patch };
      saveSettings(next).catch((e) => console.error("save settings failed", e));
      return next;
    });
  }, []);

  return { settings, update };
}
