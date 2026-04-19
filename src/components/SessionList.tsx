import type { Session } from "../types/session";
import { SessionRow } from "./SessionRow";

interface Props {
  sessions: Session[];
  onHover?: (session: Session | null) => void;
  manageMode?: boolean;
  selected?: Set<string>;
  eligibleIds?: Set<string>;
  onToggleSelect?: (id: string) => void;
}

export function SessionList({
  sessions,
  onHover,
  manageMode,
  selected,
  eligibleIds,
  onToggleSelect,
}: Props) {
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
          manageMode={manageMode}
          selected={selected?.has(s.id)}
          eligible={eligibleIds?.has(s.id) ?? false}
          onToggleSelect={onToggleSelect}
        />
      ))}
    </div>
  );
}
