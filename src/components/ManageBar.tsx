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
  { value: "all", label: "All" },
  { value: "7d", label: "7d+" },
  { value: "30d", label: "30d+" },
  { value: "90d", label: "90d+" },
  { value: "180d", label: "180d+" },
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
          placeholder="Search (title / path)"
          className="search"
          value={filter.query}
          onChange={(e) => onFilterChange({ ...filter, query: e.target.value })}
        />
        <button className="toggle" onClick={onToggleManage}>
          {manageMode ? "Done" : "Manage"}
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
              <option value="all">All CLIs</option>
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
              Selected {selectedCount} / Eligible {eligibleCount}
              <span className="hint-inline"> · Last 7 days are protected</span>
            </span>
            <div className="spacer" />
            <button onClick={onSelectAllEligible} disabled={eligibleCount === 0}>
              Select all
            </button>
            <button onClick={onClearSelection} disabled={selectedCount === 0}>
              Clear
            </button>
            <button
              className="danger"
              onClick={onBulkDelete}
              disabled={selectedCount === 0}
            >
              Move {selectedCount} to Trash
            </button>
          </div>
        </>
      )}
    </div>
  );
}
