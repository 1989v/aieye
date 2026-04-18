export type CliKind = "claude" | "codex";

export type SessionState = "running" | "recent" | "stale";

export type TerminalApp = "terminal" | "iterm2" | "alacritty" | "kitty";

export interface Session {
  id: string;
  cli: CliKind;
  title: string;
  project_path: string | null;
  git_branch: string | null;
  jsonl_path: string;
  last_activity: string;
  message_count: number | null;
  state: SessionState;
}
