import type { SubagentRow as SubagentRowData } from "../types/session";

function stateDot(state: SubagentRowData["state"]): string {
  switch (state) {
    case "running": return "●";
    case "completed": return "✓";
    case "errored": return "✕";
    default: return "○";
  }
}

function relativeTime(iso?: string | null): string {
  if (!iso) return "";
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

interface Props {
  row: SubagentRowData;
}

export function SubagentRow({ row }: Props) {
  const time = row.finished_at ?? row.started_at;
  return (
    <div className={`subagent-row state-${row.state}`}>
      <div className="subagent-line1">
        <span className="subagent-glyph">↳</span>
        <span className="subagent-dot" aria-label={row.state}>
          {stateDot(row.state)}
        </span>
        <span className="subagent-name">{row.name}</span>
        {row.description && <span className="subagent-desc">{row.description}</span>}
        <span className="subagent-time">{relativeTime(time)}</span>
      </div>
      {row.last_text && <div className="subagent-last-text">{row.last_text}</div>}
    </div>
  );
}
