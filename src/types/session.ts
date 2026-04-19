export type CliKind = "claude" | "codex";

export type SessionState = "running" | "recent" | "stale";

export type TerminalApp = "terminal" | "iterm2" | "alacritty" | "kitty";

export type HostKind = "terminal" | "iterm2" | "vscode" | "jetbrains" | "other";

export type Activity = "generating" | "idle";

export interface RunningInfo {
  pid: number;
  tty: string;
  host_kind: HostKind;
  host_name: string | null;
  activity?: Activity | null;
}

export interface SessionPreviewInline {
  last_user?: string | null;
  last_assistant?: string | null;
}

export type TurnRole = "user" | "assistant";

export interface Turn {
  role: TurnRole;
  text: string;
  timestamp?: string | null;
}

export interface SessionPreview {
  last_user?: string | null;
  last_assistant?: string | null;
  recent_turns: Turn[];
}

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
  running?: RunningInfo | null;
  finished?: boolean;
  inline_preview?: SessionPreviewInline | null;
}
