import { useMemo, useState } from "react";
import "./styles.css";
import { useSessions } from "./hooks/useSessions";
import { SessionList } from "./components/SessionList";
import { SettingsMenu } from "./components/SettingsMenu";
import { PreviewPane } from "./components/PreviewPane";
import { ManageBar, type FilterState } from "./components/ManageBar";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { archiveSessionsBulk } from "./ipc/tauri";
import type { Session } from "./types/session";
import { useSettings } from "./hooks/useSettings";

const SAFETY_DAYS = 7;

function daysAgo(iso: string): number {
  return (Date.now() - new Date(iso).getTime()) / 86400000;
}

function ageThresholdDays(age: FilterState["age"]): number {
  switch (age) {
    case "7d":
      return 7;
    case "30d":
      return 30;
    case "90d":
      return 90;
    case "180d":
      return 180;
    default:
      return 0;
  }
}

export default function App() {
  const { sessions, error } = useSessions();
  const { settings } = useSettings();
  const [hovered, setHovered] = useState<Session | null>(null);
  const [pinned, setPinned] = useState<Session | null>(null);
  const [focusKey, setFocusKey] = useState(0);

  const previewTarget = pinned ?? hovered;

  const pinSessionForReply = (s: Session) => {
    setPinned(s);
    setFocusKey((k) => k + 1);
  };
  const [manageMode, setManageMode] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState<FilterState>({
    query: "",
    cli: "all",
    age: "all",
  });
  const [confirmBulk, setConfirmBulk] = useState(false);
  const [bulkResult, setBulkResult] = useState<{
    archived: number;
    skipped: number;
  } | null>(null);

  const filtered = useMemo(() => {
    if (!sessions) return [];
    const q = filter.query.trim().toLowerCase();
    const ageThreshold = ageThresholdDays(filter.age);
    return sessions.filter((s) => {
      if (filter.cli !== "all" && s.cli !== filter.cli) return false;
      if (ageThreshold > 0 && daysAgo(s.last_activity) < ageThreshold) return false;
      if (q) {
        const hay = `${s.title} ${s.project_path ?? ""}`.toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    });
  }, [sessions, filter]);

  interface RepoGroup {
    repoName: string;
    sessions: Session[];
    latestActivity: string;
  }

  const grouped = useMemo<RepoGroup[]>(() => {
    if (!settings?.group_by_repo) {
      return [{ repoName: "", sessions: filtered, latestActivity: "" }];
    }
    const map = new Map<string, Session[]>();
    for (const s of filtered) {
      const key = s.repo_name || "(no project)";
      const arr = map.get(key);
      if (arr) arr.push(s);
      else map.set(key, [s]);
    }
    const groups: RepoGroup[] = [];
    for (const [name, list] of map) {
      list.sort((a, b) => b.last_activity.localeCompare(a.last_activity));
      groups.push({ repoName: name, sessions: list, latestActivity: list[0].last_activity });
    }
    groups.sort((a, b) => {
      if (a.repoName === "(no project)") return 1;
      if (b.repoName === "(no project)") return -1;
      return b.latestActivity.localeCompare(a.latestActivity);
    });
    return groups;
  }, [filtered, settings?.group_by_repo]);

  const [collapsedRepos, setCollapsedRepos] = useState<Set<string>>(new Set());
  const toggleRepo = (name: string) => {
    setCollapsedRepos((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const eligibleIds = useMemo(() => {
    const ids = new Set<string>();
    for (const s of filtered) {
      if (daysAgo(s.last_activity) >= SAFETY_DAYS && !s.running) {
        ids.add(s.id);
      }
    }
    return ids;
  }, [filtered]);

  const selectedPaths = useMemo(() => {
    if (!sessions) return [];
    return sessions.filter((s) => selected.has(s.id)).map((s) => s.jsonl_path);
  }, [sessions, selected]);

  const toggleSelect = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAllEligible = () => setSelected(new Set(eligibleIds));
  const clearSelection = () => setSelected(new Set());

  const runBulk = async () => {
    const res = await archiveSessionsBulk(selectedPaths);
    setBulkResult({
      archived: res.archived.length,
      skipped: res.skipped_recent.length,
    });
    clearSelection();
  };

  return (
    <div className="app split">
      <div className="left">
        <div className="header">
          <span className="brand">
            <img
              className="brand-icon"
              src="/panel-eye.png"
              srcSet="/panel-eye.png 1x, /panel-eye@2x.png 2x"
              alt=""
              width={18}
              height={18}
            />
            aieye
          </span>
          {sessions && <span className="count">{filtered.length}</span>}
        </div>
        <ManageBar
          manageMode={manageMode}
          onToggleManage={() => {
            setManageMode((m) => !m);
            clearSelection();
          }}
          filter={filter}
          onFilterChange={setFilter}
          selectedCount={selected.size}
          eligibleCount={eligibleIds.size}
          onSelectAllEligible={selectAllEligible}
          onClearSelection={clearSelection}
          onBulkDelete={() => setConfirmBulk(true)}
        />
        {error && <div className="error">{error}</div>}
        {sessions === null && !error && <div className="empty">Scanning…</div>}
        {sessions && (
          <SessionList
            groups={grouped}
            collapsedRepos={collapsedRepos}
            onToggleRepo={toggleRepo}
            groupByRepo={settings?.group_by_repo ?? true}
            onHover={setHovered}
            manageMode={manageMode}
            selected={selected}
            eligibleIds={eligibleIds}
            onToggleSelect={toggleSelect}
            onPinReply={pinSessionForReply}
          />
        )}
        <SettingsMenu />
      </div>
      <div className="right">
        <PreviewPane
          session={previewTarget}
          focusReplyKey={focusKey}
          onUnpin={pinned ? () => setPinned(null) : undefined}
        />
      </div>

      <ConfirmDialog
        open={confirmBulk}
        title="Move selected to Trash"
        message={`Moving ${selected.size} session(s) to Trash.\n\nSessions active within the last 7 days are automatically skipped by a backend safeguard.\nRecoverable from Finder Trash.`}
        confirmLabel={`Move ${selected.size}`}
        danger
        onCancel={() => setConfirmBulk(false)}
        onConfirm={() => {
          setConfirmBulk(false);
          runBulk();
        }}
      />
      <ConfirmDialog
        open={bulkResult !== null}
        title="Done"
        message={
          bulkResult
            ? `Moved ${bulkResult.archived}.${bulkResult.skipped > 0 ? `\n${bulkResult.skipped} skipped (recent activity, protected).` : ""}`
            : ""
        }
        confirmLabel="OK"
        cancelLabel=""
        onCancel={() => setBulkResult(null)}
        onConfirm={() => setBulkResult(null)}
      />
    </div>
  );
}
