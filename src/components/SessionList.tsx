import type { Session } from "../types/session";
import { SessionRow } from "./SessionRow";
import { RepoGroupHeader } from "./RepoGroupHeader";

interface RepoGroup {
  repoName: string;
  sessions: Session[];
  latestActivity: string;
}

interface Props {
  groups: RepoGroup[];
  collapsedRepos: Set<string>;
  onToggleRepo: (name: string) => void;
  expandedRepos: Set<string>;
  onToggleRepoExpansion: (name: string) => void;
  groupByRepo: boolean;
  onHover?: (session: Session | null) => void;
  manageMode?: boolean;
  selected?: Set<string>;
  eligibleIds?: Set<string>;
  onToggleSelect?: (id: string) => void;
  onPinReply?: (session: Session) => void;
}

const DEFAULT_VISIBLE = 5;

function relativeTime(iso: string): string {
  if (!iso) return "";
  const delta = (Date.now() - new Date(iso).getTime()) / 1000;
  if (delta < 60) return `${Math.floor(delta)}s ago`;
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
  return `${Math.floor(delta / 86400)}d ago`;
}

export function SessionList({
  groups,
  collapsedRepos,
  onToggleRepo,
  expandedRepos,
  onToggleRepoExpansion,
  groupByRepo,
  onHover,
  manageMode,
  selected,
  eligibleIds,
  onToggleSelect,
  onPinReply,
}: Props) {
  const totalSessions = groups.reduce((sum, g) => sum + g.sessions.length, 0);
  if (totalSessions === 0) {
    return <div className="empty">No sessions yet.</div>;
  }
  return (
    <div className="session-list" onMouseLeave={() => onHover?.(null)}>
      {groups.map((g) => {
        const isCollapsed = collapsedRepos.has(g.repoName);
        const showHeader = groupByRepo && (groups.length > 1 || g.repoName !== "");
        const isExpanded = expandedRepos.has(g.repoName);
        const visibleSessions =
          isExpanded || g.sessions.length <= DEFAULT_VISIBLE
            ? g.sessions
            : g.sessions.slice(0, DEFAULT_VISIBLE);
        const hiddenCount = g.sessions.length - visibleSessions.length;
        return (
          <div key={g.repoName || "_flat"} className="repo-group">
            {showHeader && (
              <RepoGroupHeader
                repoName={g.repoName}
                sessionCount={g.sessions.length}
                latestRelative={relativeTime(g.latestActivity)}
                collapsed={isCollapsed}
                onToggle={() => onToggleRepo(g.repoName)}
              />
            )}
            {!isCollapsed &&
              visibleSessions.map((s) => (
                <SessionRow
                  key={`${s.cli}-${s.id}`}
                  session={s}
                  onHover={onHover}
                  manageMode={manageMode}
                  selected={selected?.has(s.id)}
                  eligible={eligibleIds?.has(s.id) ?? false}
                  onToggleSelect={onToggleSelect}
                  onPinReply={onPinReply}
                />
              ))}
            {!isCollapsed && (hiddenCount > 0 || isExpanded) && (
              <button
                className="repo-show-more"
                onClick={() => onToggleRepoExpansion(g.repoName)}
              >
                {isExpanded ? "Show less" : `Show ${hiddenCount} more`}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
