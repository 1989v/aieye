interface Props {
  repoName: string;
  sessionCount: number;
  latestRelative: string;
  collapsed: boolean;
  onToggle: () => void;
}

export function RepoGroupHeader({
  repoName,
  sessionCount,
  latestRelative,
  collapsed,
  onToggle,
}: Props) {
  return (
    <button className="repo-group-header" onClick={onToggle} aria-expanded={!collapsed}>
      <span className={`caret ${collapsed ? "right" : "down"}`}>{collapsed ? "▸" : "▾"}</span>
      <span className="repo-name">{repoName}</span>
      <span className="repo-count">{sessionCount}</span>
      <span className="repo-latest">{latestRelative}</span>
    </button>
  );
}
