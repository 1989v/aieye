import { invoke } from "@tauri-apps/api/core";
import type { Session, TerminalApp } from "../types/session";
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
