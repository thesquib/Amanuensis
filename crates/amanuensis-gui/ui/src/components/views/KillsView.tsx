import { useMemo, useCallback, useState, useEffect } from "react";
import { createColumnHelper, type SortingState } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { StatCard } from "../shared/StatCard";
import { CreatureImage } from "../shared/CreatureImage";
import { KillDetailModal } from "../shared/KillDetailModal";
import { KillsFilterBar, type KillsFilterState } from "../shared/KillsFilterBar";
import { formatDate, formatTwoHourWindow } from "../../lib/dateUtils";
import { computeKillStats } from "../../lib/killStats";
import { getKillFrequency } from "../../lib/commands";
import type { Kill } from "../../types";

const columnHelper = createColumnHelper<Kill>();

export function KillsView() {
  const { kills, viewStates, setViewSorting, setViewFilter } = useStore();
  const viewState = viewStates["kills"];
  const [selectedKill, setSelectedKill] = useState<Kill | null>(null);
  const [filter, setFilter] = useState<KillsFilterState>({
    families: new Set(),
    rarities: new Set(),
    seasonal: false,
  });
  const byName = useStore((s) => s.bestiaryByName);
  const selectedCharacterId = useStore((s) => s.selectedCharacterId);
  const killFrequency = useStore((s) => s.killFrequency);
  const killFrequencyCharId = useStore((s) => s.killFrequencyCharId);
  const setKillFrequency = useStore((s) => s.setKillFrequency);

  useEffect(() => {
    if (selectedCharacterId == null) return;
    if (killFrequencyCharId === selectedCharacterId) return;
    getKillFrequency(selectedCharacterId, true)
      .then((rows) => setKillFrequency(selectedCharacterId, rows))
      .catch((err) => console.error("Failed to load kill frequency:", err));
  }, [selectedCharacterId, killFrequencyCharId, setKillFrequency]);

  const columns = useMemo(
    () => [
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
      columnHelper.accessor("killed_by_count", {
        header: "Killed By",
        cell: (info) => info.getValue().toLocaleString(),
      }),
      columnHelper.accessor("creature_value", {
        header: "Value",
        cell: (info) => info.getValue(),
      }),
      columnHelper.accessor("date_first", {
        header: "First Kill",
        cell: (info) => formatDate(info.getValue()),
      }),
      columnHelper.accessor("date_last", {
        header: "Last Kill",
        cell: (info) => formatDate(info.getValue()),
      }),
      columnHelper.accessor(
        (row) => killFrequency[row.creature_name]?.best_day_count ?? 0,
        {
          id: "best_day",
          header: "Best Day",
          cell: (info) => {
            const freq = killFrequency[info.row.original.creature_name];
            const count = freq?.best_day_count ?? 0;
            if (count === 0) return <span>-</span>;
            const tooltip = freq?.best_day_date
              ? `${count.toLocaleString()} on ${freq.best_day_date} — the most in any single day`
              : undefined;
            return <span title={tooltip}>{count.toLocaleString()}</span>;
          },
        },
      ),
      columnHelper.accessor(
        (row) => killFrequency[row.creature_name]?.best_2h_count ?? 0,
        {
          id: "best_2h",
          header: "Best 2h",
          cell: (info) => {
            const freq = killFrequency[info.row.original.creature_name];
            const count = freq?.best_2h_count ?? 0;
            if (count === 0) return <span>-</span>;
            const tooltip = freq?.best_2h_start
              ? `${count.toLocaleString()} on ${formatTwoHourWindow(freq.best_2h_start)} — the most in any 2-hour window`
              : undefined;
            return <span title={tooltip}>{count.toLocaleString()}</span>;
          },
        },
      ),
    ],
    [killFrequency],
  );

  const sorting = viewState?.sorting ?? [{ id: "date_last", desc: true }];
  const globalFilter = viewState?.globalFilter ?? "";
  const onSortingChange = useCallback(
    (s: SortingState) => setViewSorting("kills", s),
    [setViewSorting],
  );
  const onGlobalFilterChange = useCallback(
    (f: string) => setViewFilter("kills", f),
    [setViewFilter],
  );

  const stats = useMemo(() => computeKillStats(kills), [kills]);

  const visibleKills = useMemo(() => {
    if (filter.families.size === 0 && filter.rarities.size === 0 && !filter.seasonal) {
      return kills;
    }
    return kills.filter((k) => {
      const e = byName[k.creature_name];
      if (filter.families.size > 0) {
        if (!e?.family_canonical || !filter.families.has(e.family_canonical)) return false;
      }
      if (filter.rarities.size > 0) {
        if (!e?.rarity_canonical || !filter.rarities.has(e.rarity_canonical)) return false;
      }
      if (filter.seasonal) {
        if (!e?.is_seasonal) return false;
      }
      return true;
    });
  }, [kills, filter, byName]);

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
        <KillsFilterBar kills={kills} value={filter} onChange={setFilter} />
        <DataTable
          data={visibleKills}
          columns={columns}
          enableSearch
          searchPlaceholder="Search creatures..."
          sorting={sorting}
          onSortingChange={onSortingChange}
          globalFilter={globalFilter}
          onGlobalFilterChange={onGlobalFilterChange}
          onRowClick={(row) => setSelectedKill(row)}
        />
      </div>
      {selectedKill && (
        <KillDetailModal kill={selectedKill} onClose={() => setSelectedKill(null)} />
      )}
    </div>
  );
}
