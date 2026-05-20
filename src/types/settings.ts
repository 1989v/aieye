import type { TerminalApp } from "./session";

export type ReplyMode = "paste" | "headless";

export interface Settings {
  preferred_terminal: TerminalApp;
  recent_threshold_minutes: number;
  reply_mode: ReplyMode;
  group_by_repo: boolean;
}
