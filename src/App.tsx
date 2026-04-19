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
  const [hovered, setHovered] = useState<Session | null>(null);
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
          👁 aieye
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
            sessions={filtered}
            onHover={setHovered}
            manageMode={manageMode}
            selected={selected}
            eligibleIds={eligibleIds}
            onToggleSelect={toggleSelect}
          />
        )}
        <SettingsMenu />
      </div>
      <div className="right">
        <PreviewPane session={hovered} />
      </div>

      <ConfirmDialog
        open={confirmBulk}
        title="선택한 세션을 휴지통으로"
        message={`${selected.size}개 세션을 휴지통으로 이동합니다.\n\n최근 7일 이내 활동 세션은 백엔드 안전장치로 자동 제외됩니다.\nFinder 휴지통에서 복구 가능.`}
        confirmLabel={`${selected.size}개 이동`}
        danger
        onCancel={() => setConfirmBulk(false)}
        onConfirm={() => {
          setConfirmBulk(false);
          runBulk();
        }}
      />
      <ConfirmDialog
        open={bulkResult !== null}
        title="완료"
        message={
          bulkResult
            ? `${bulkResult.archived}개 이동 완료.${bulkResult.skipped > 0 ? `\n${bulkResult.skipped}개는 최근 활동으로 보호되어 skip.` : ""}`
            : ""
        }
        confirmLabel="확인"
        cancelLabel=""
        onCancel={() => setBulkResult(null)}
        onConfirm={() => setBulkResult(null)}
      />
    </div>
  );
}
