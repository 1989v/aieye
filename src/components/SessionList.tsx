import type { Session } from "../types/session";
import { SessionRow } from "./SessionRow";

export function SessionList({ sessions }: { sessions: Session[] }) {
  if (sessions.length === 0) {
    return <div className="empty">No sessions yet.</div>;
  }
  return (
    <div className="session-list">
      {sessions.map((s) => (
        <SessionRow key={`${s.cli}-${s.id}`} session={s} />
      ))}
    </div>
  );
}
