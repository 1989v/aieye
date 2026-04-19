import { invoke } from "@tauri-apps/api/core";
import type { CliKind, Session, SessionPreview, TerminalApp } from "../types/session";
import type { Settings } from "../types/settings";

export async function listSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_sessions");
}

export async function resumeSession(session: Session, terminal?: TerminalApp): Promise<void> {
  await invoke("resume_session", { session, terminal });
}

export async function resumeSessionForceNew(
  session: Session,
  terminal?: TerminalApp,
): Promise<void> {
  await invoke("resume_session_force_new", { session, terminal });
}

export async function revealInFinder(path: string): Promise<void> {
  await invoke("reveal_in_finder", { path });
}

export async function listInstalledTerminals(): Promise<TerminalApp[]> {
  return invoke<TerminalApp[]>("list_installed_terminals");
}

export async function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function setSettings(settings: Settings): Promise<void> {
  await invoke("set_settings", { settings });
}

export async function acknowledgeFinished(sessionId: string): Promise<void> {
  await invoke("acknowledge_finished", { sessionId });
}

export async function getSessionPreview(
  jsonlPath: string,
  cli: CliKind,
): Promise<SessionPreview> {
  return invoke<SessionPreview>("get_session_preview", { jsonlPath, cli });
}

export async function archiveSessionFile(jsonlPath: string): Promise<void> {
  await invoke("archive_session_file", { jsonlPath });
}

export interface BulkArchiveResult {
  archived: string[];
  skipped_recent: string[];
  errors: string[];
}

export async function archiveSessionsBulk(paths: string[]): Promise<BulkArchiveResult> {
  return invoke<BulkArchiveResult>("archive_sessions_bulk", { paths });
}
