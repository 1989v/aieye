import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { listSessions } from "../ipc/tauri";
import type { Session } from "../types/session";

export function useSessions() {
  const [sessions, setSessions] = useState<Session[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    listSessions()
      .then((s) => setSessions(s))
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    // 초기 로드
    refresh();
    // 백엔드가 패널 show 할 때마다 재조회 → 최신 상태 반영
    const unlistenPromise = listen<void>("panel-shown", () => refresh());
    return () => {
      unlistenPromise.then((un) => un()).catch(() => {});
    };
  }, [refresh]);

  return { sessions, error, refresh };
}
