import { useMemo } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { DataTable } from "../../shared/DataTable";
import { CreatureImage } from "../../shared/CreatureImage";
import { useStore } from "../../../lib/store";
import type { StudyRecord, StudyState } from "../../../lib/rangerStats";

function StatusBadge({ state }: { state: StudyState }) {
  switch (state.status) {
    case "completed":
      return (
        <span className="inline-flex items-center gap-1 rounded-full bg-green-500/15 px-2 py-0.5 text-xs font-medium text-green-400">
          <span className="text-[10px]">&#10003;</span> Done
        </span>
      );
    case "in_progress":
      return (
        <span className="inline-flex items-center gap-1 rounded-full bg-yellow-500/15 px-2 py-0.5 text-xs font-medium text-yellow-400">
          <span className="h-1.5 w-1.5 rounded-full bg-yellow-400" /> {state.message_count}
        </span>
      );
    case "abandoned":
      return (
        <span className="inline-flex items-center rounded-full bg-red-500/15 px-2 py-0.5 text-xs font-medium text-red-400">
          Abandoned
        </span>
      );
    default:
      return <span className="text-[var(--color-text-muted)]">—</span>;
  }
}

const columnHelper = createColumnHelper<StudyRecord>();

const columns = [
  columnHelper.accessor("creature_name", {
    header: "Creature",
    cell: (info) => (
      <div className="flex items-center gap-2">
        <CreatureImage creatureName={info.getValue()} className="h-6 w-6" />
        <span>{info.getValue()}</span>
      </div>
    ),
  }),
  columnHelper.accessor("family", {
    header: "Family",
    cell: (info) => info.getValue() || "—",
  }),
  columnHelper.accessor("value", {
    header: "Value",
    cell: (info) => info.getValue() > 0 ? info.getValue().toLocaleString() : "—",
  }),
  columnHelper.accessor("movements", {
    header: "Movements",
    cell: (info) => <StatusBadge state={info.getValue()} />,
    sortingFn: (rowA, rowB) => {
      const order: Record<string, number> = { completed: 3, in_progress: 2, abandoned: 1, none: 0 };
      return (order[rowA.original.movements.status] ?? 0) - (order[rowB.original.movements.status] ?? 0);
    },
  }),
  columnHelper.accessor("befriend", {
    header: "Befriend",
    cell: (info) => <StatusBadge state={info.getValue()} />,
    sortingFn: (rowA, rowB) => {
      const order: Record<string, number> = { completed: 3, in_progress: 2, abandoned: 1, none: 0 };
      return (order[rowA.original.befriend.status] ?? 0) - (order[rowB.original.befriend.status] ?? 0);
    },
  }),
  columnHelper.accessor("morph", {
    header: "Morph",
    cell: (info) => <StatusBadge state={info.getValue()} />,
    sortingFn: (rowA, rowB) => {
      const order: Record<string, number> = { completed: 3, in_progress: 2, abandoned: 1, none: 0 };
      return (order[rowA.original.morph.status] ?? 0) - (order[rowB.original.morph.status] ?? 0);
    },
  }),
  columnHelper.accessor("duvin_cost", {
    header: "Duvin Cost",
    cell: (info) => info.getValue() > 0 ? info.getValue() : "—",
  }),
];

interface StudiesPanelProps {
  studies: StudyRecord[];
}

export function StudiesPanel({ studies }: StudiesPanelProps) {
  const { rangerStatsViewState, setRangerStatsViewState } = useStore();
  const searchQuery = rangerStatsViewState.searchQuery;

  const filtered = useMemo(() => {
    if (!searchQuery) return studies;
    const q = searchQuery.toLowerCase();
    return studies.filter(
      (s) =>
        s.creature_name.toLowerCase().includes(q) ||
        s.family.toLowerCase().includes(q),
    );
  }, [studies, searchQuery]);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-3">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setRangerStatsViewState({ searchQuery: e.target.value })}
          placeholder="Search creatures or families..."
          className="w-64 rounded border border-[var(--color-border)] bg-[var(--color-sidebar)] px-3 py-1.5 text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
        />
        <span className="ml-3 text-xs text-[var(--color-text-muted)]">
          {filtered.length} creature{filtered.length !== 1 ? "s" : ""}
        </span>
      </div>
      <DataTable data={filtered} columns={columns} />
    </div>
  );
}
