import type { CliKind } from "../types/session";

export type AgeFilter = "all" | "7d" | "30d" | "90d" | "180d";

export interface FilterState {
  query: string;
  cli: "all" | CliKind;
  age: AgeFilter;
}

interface Props {
  manageMode: boolean;
  onToggleManage: () => void;
  filter: FilterState;
  onFilterChange: (f: FilterState) => void;
  selectedCount: number;
  eligibleCount: number;
  onSelectAllEligible: () => void;
  onClearSelection: () => void;
  onBulkDelete: () => void;
}

const AGE_OPTIONS: { value: AgeFilter; label: string }[] = [
  { value: "all", label: "전체" },
  { value: "7d", label: "7일 이상" },
  { value: "30d", label: "30일 이상" },
  { value: "90d", label: "90일 이상" },
  { value: "180d", label: "180일 이상" },
];

export function ManageBar({
  manageMode,
  onToggleManage,
  filter,
  onFilterChange,
  selectedCount,
  eligibleCount,
  onSelectAllEligible,
  onClearSelection,
  onBulkDelete,
}: Props) {
  return (
    <div className="manage-bar">
      <div className="row">
        <input
          type="text"
          placeholder="검색 (제목 / 경로)"
          className="search"
          value={filter.query}
          onChange={(e) => onFilterChange({ ...filter, query: e.target.value })}
        />
        <button className="toggle" onClick={onToggleManage}>
          {manageMode ? "완료" : "정리"}
        </button>
      </div>
      {manageMode && (
        <>
          <div className="row small">
            <select
              value={filter.cli}
              onChange={(e) =>
                onFilterChange({ ...filter, cli: e.target.value as FilterState["cli"] })
              }
            >
              <option value="all">모든 CLI</option>
              <option value="claude">Claude</option>
              <option value="codex">Codex</option>
            </select>
            <select
              value={filter.age}
              onChange={(e) =>
                onFilterChange({ ...filter, age: e.target.value as AgeFilter })
              }
            >
              {AGE_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
          </div>
          <div className="row small actions">
            <span className="info">
              선택 {selectedCount} / 대상 {eligibleCount}
              <span className="hint-inline"> · 최근 7일 이내 세션은 보호됨</span>
            </span>
            <div className="spacer" />
            <button onClick={onSelectAllEligible} disabled={eligibleCount === 0}>
              전체 선택
            </button>
            <button onClick={onClearSelection} disabled={selectedCount === 0}>
              해제
            </button>
            <button
              className="danger"
              onClick={onBulkDelete}
              disabled={selectedCount === 0}
            >
              {selectedCount}개 휴지통 이동
            </button>
          </div>
        </>
      )}
    </div>
  );
}
