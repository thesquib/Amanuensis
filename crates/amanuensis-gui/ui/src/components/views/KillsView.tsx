import { useMemo } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { StatCard } from "../shared/StatCard";
import type { Kill } from "../../types";

const columnHelper = createColumnHelper<Kill>();

const columns = [
  columnHelper.accessor("creature_name", {
    header: "Creature",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor(
    (row) =>
      row.killed_count +
      row.slaughtered_count +
      row.vanquished_count +
      row.dispatched_count,
    {
      id: "solo",
      header: "Solo",
      cell: (info) => info.getValue().toLocaleString(),
    },
  ),
  columnHelper.accessor(
    (row) =>
      row.assisted_kill_count +
      row.assisted_slaughter_count +
      row.assisted_vanquish_count +
      row.assisted_dispatch_count,
    {
      id: "assisted",
      header: "Assisted",
      cell: (info) => info.getValue().toLocaleString(),
    },
  ),
  columnHelper.accessor(
    (row) =>
      row.killed_count +
      row.slaughtered_count +
      row.vanquished_count +
      row.dispatched_count +
      row.assisted_kill_count +
      row.assisted_slaughter_count +
      row.assisted_vanquish_count +
      row.assisted_dispatch_count,
    {
      id: "total",
      header: "Total",
      cell: (info) => info.getValue().toLocaleString(),
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
    header: "First",
    cell: (info) => {
      const val = info.getValue();
      return val ? val.split(" ")[0] : "";
    },
  }),
  columnHelper.accessor("date_last", {
    header: "Last",
    cell: (info) => {
      const val = info.getValue();
      return val ? val.split(" ")[0] : "";
    },
  }),
];

export function KillsView() {
  const { kills } = useStore();

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
    const highest = kills.reduce(
      (best, k) => {
        const total =
          k.killed_count +
          k.slaughtered_count +
          k.vanquished_count +
          k.dispatched_count +
          k.assisted_kill_count +
          k.assisted_slaughter_count +
          k.assisted_vanquish_count +
          k.assisted_dispatch_count;
        return total > best.count
          ? { name: k.creature_name, count: total }
          : best;
      },
      { name: "None", count: 0 },
    );
    return { totalSolo, totalAssisted, totalKilledBy, highest };
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
          label="Killed By"
          value={stats.totalKilledBy.toLocaleString()}
        />
        <StatCard
          label="Top Creature"
          value={stats.highest.name}
          sub={`${stats.highest.count} kills`}
        />
      </div>
      <div className="min-h-0 flex-1">
        <DataTable
          data={kills}
          columns={columns}
          enableSearch
          searchPlaceholder="Search creatures..."
        />
      </div>
    </div>
  );
}
