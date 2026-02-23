import { useMemo, useCallback } from "react";
import { createColumnHelper, type SortingState } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { StatCard } from "../shared/StatCard";
import { CreatureImage } from "../shared/CreatureImage";
import type { Kill } from "../../types";

const columnHelper = createColumnHelper<Kill>();

function formatDate(val: string | null) {
  return val ? val.split(" ")[0] : "";
}

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
  columnHelper.accessor(
    (row) => row.vanquished_count + row.assisted_vanquish_count,
    {
      id: "vanquished",
      header: "Vanquished",
      cell: (info) => {
        const row = info.row.original;
        const total = row.vanquished_count + row.assisted_vanquish_count;
        const date = row.date_last_vanquished;
        return (
          <span title={date ? `Last vanquish: ${formatDate(date)}` : undefined}>
            {total.toLocaleString()}
            {row.vanquished_count > 0 && (
              <span className="text-[var(--color-text-muted)]"> ({row.vanquished_count})</span>
            )}
          </span>
        );
      },
    },
  ),
  columnHelper.accessor(
    (row) => row.slaughtered_count + row.assisted_slaughter_count,
    {
      id: "slaughtered",
      header: "Slaughtered",
      cell: (info) => {
        const row = info.row.original;
        const total = row.slaughtered_count + row.assisted_slaughter_count;
        const date = row.date_last_slaughtered;
        return (
          <span title={date ? `Last slaughter: ${formatDate(date)}` : undefined}>
            {total.toLocaleString()}
            {row.slaughtered_count > 0 && total !== row.slaughtered_count && (
              <span className="text-[var(--color-text-muted)]"> ({row.slaughtered_count})</span>
            )}
          </span>
        );
      },
    },
  ),
  columnHelper.accessor(
    (row) => row.killed_count + row.assisted_kill_count,
    {
      id: "killed",
      header: "Killed",
      cell: (info) => {
        const row = info.row.original;
        const total = row.killed_count + row.assisted_kill_count;
        const date = row.date_last_killed;
        return (
          <span title={date ? `Last kill: ${formatDate(date)}` : undefined}>
            {total.toLocaleString()}
            {row.killed_count > 0 && total !== row.killed_count && (
              <span className="text-[var(--color-text-muted)]"> ({row.killed_count})</span>
            )}
          </span>
        );
      },
    },
  ),
  columnHelper.accessor(
    (row) => row.dispatched_count + row.assisted_dispatch_count,
    {
      id: "dispatched",
      header: "Dispatched",
      cell: (info) => {
        const row = info.row.original;
        const total = row.dispatched_count + row.assisted_dispatch_count;
        const date = row.date_last_dispatched;
        return (
          <span title={date ? `Last dispatch: ${formatDate(date)}` : undefined}>
            {total.toLocaleString()}
            {row.dispatched_count > 0 && total !== row.dispatched_count && (
              <span className="text-[var(--color-text-muted)]"> ({row.dispatched_count})</span>
            )}
          </span>
        );
      },
    },
  ),
  columnHelper.accessor("killed_by_count", {
    header: "Killed By",
    cell: (info) => info.getValue().toLocaleString(),
  }),
  columnHelper.accessor("creature_value", {
    header: "Value",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("date_first", {
    header: "First Seen",
    cell: (info) => formatDate(info.getValue()),
  }),
  columnHelper.accessor("date_last", {
    header: "Last Seen",
    cell: (info) => formatDate(info.getValue()),
  }),
];

export function KillsView() {
  const { kills, viewStates, setViewSorting, setViewFilter } = useStore();
  const viewState = viewStates["kills"];
  const sorting = viewState?.sorting ?? [];
  const globalFilter = viewState?.globalFilter ?? "";
  const onSortingChange = useCallback(
    (s: SortingState) => setViewSorting("kills", s),
    [setViewSorting],
  );
  const onGlobalFilterChange = useCallback(
    (f: string) => setViewFilter("kills", f),
    [setViewFilter],
  );

  const stats = useMemo(() => {
    const totalSolo = kills.reduce(
      (s, k) =>
        s +
        k.killed_count +
        k.slaughtered_count +
        k.vanquished_count +
        k.dispatched_count,
      0,
    );
    const totalAssisted = kills.reduce(
      (s, k) =>
        s +
        k.assisted_kill_count +
        k.assisted_slaughter_count +
        k.assisted_vanquish_count +
        k.assisted_dispatch_count,
      0,
    );
    const totalKilledBy = kills.reduce((s, k) => s + k.killed_by_count, 0);
    const totalVanquished = kills.reduce((s, k) => s + k.vanquished_count + k.assisted_vanquish_count, 0);
    const totalSlaughtered = kills.reduce((s, k) => s + k.slaughtered_count + k.assisted_slaughter_count, 0);
    const totalKilled = kills.reduce((s, k) => s + k.killed_count + k.assisted_kill_count, 0);
    const totalDispatched = kills.reduce((s, k) => s + k.dispatched_count + k.assisted_dispatch_count, 0);
    return { totalSolo, totalAssisted, totalKilledBy, totalVanquished, totalSlaughtered, totalKilled, totalDispatched };
  }, [kills]);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
        <StatCard
          label="Solo Kills"
          value={stats.totalSolo.toLocaleString()}
        />
        <StatCard
          label="Assisted"
          value={stats.totalAssisted.toLocaleString()}
        />
        <StatCard
          label="Vanquished"
          value={stats.totalVanquished.toLocaleString()}
        />
        <StatCard
          label="Slaughtered"
          value={stats.totalSlaughtered.toLocaleString()}
        />
        <StatCard
          label="Killed"
          value={stats.totalKilled.toLocaleString()}
        />
        <StatCard
          label="Dispatched"
          value={stats.totalDispatched.toLocaleString()}
        />
        <StatCard
          label="Killed By"
          value={stats.totalKilledBy.toLocaleString()}
        />
      </div>
      <div className="min-h-0 flex-1">
        <DataTable
          data={kills}
          columns={columns}
          enableSearch
          searchPlaceholder="Search creatures..."
          sorting={sorting}
          onSortingChange={onSortingChange}
          globalFilter={globalFilter}
          onGlobalFilterChange={onGlobalFilterChange}
        />
      </div>
    </div>
  );
}
