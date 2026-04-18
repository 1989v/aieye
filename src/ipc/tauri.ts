import { invoke } from "@tauri-apps/api/core";
import type { Session } from "../types/session";

export async function listSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_sessions");
}
