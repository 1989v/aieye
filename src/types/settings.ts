import type { TerminalApp } from "./session";

export interface Settings {
  preferred_terminal: TerminalApp;
  recent_threshold_minutes: number;
}
