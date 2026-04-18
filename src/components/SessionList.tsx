import type { Session } from "../types/session";

function relativeTime(iso: string): string {
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

function stateDot(state: Session["state"]): string {
  return state === "running" ? "🟢" : state === "recent" ? "🟡" : "🔘";
}

interface Props {
  sessions: Session[];
}

export function SessionList({ sessions }: Props) {
  if (sessions.length === 0) {
    return <div className="empty">No sessions yet.</div>;
  }
  return (
    <div className="session-list">
      {sessions.map((s) => (
        <div key={`${s.cli}-${s.id}`} className="session-row">
          <span className="state">{stateDot(s.state)}</span>
          <span className="cli">[{s.cli}]</span>
          <div className="body">
            <div className="title">{s.title}</div>
            <div className="sub">
              {s.project_path ?? "unknown path"}
              {s.git_branch && <> · {s.git_branch}</>}
              <> · {relativeTime(s.last_activity)}</>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
