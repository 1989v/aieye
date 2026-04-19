import type { Session } from "../types/session";
import { SessionRow } from "./SessionRow";

interface Props {
  sessions: Session[];
  onHover?: (session: Session | null) => void;
}

export function SessionList({ sessions, onHover }: Props) {
  if (sessions.length === 0) {
    return <div className="empty">No sessions yet.</div>;
  }
  return (
    <div className="session-list" onMouseLeave={() => onHover?.(null)}>
      {sessions.map((s) => (
        <SessionRow
          key={`${s.cli}-${s.id}`}
          session={s}
          onHover={onHover}
        />
      ))}
    </div>
  );
}
