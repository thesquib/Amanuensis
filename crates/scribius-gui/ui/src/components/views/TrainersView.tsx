import { useMemo, useState, useEffect } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import { getTrainerDbInfo } from "../../lib/commands";
import type { Trainer, TrainerInfo } from "../../types";

const columnHelper = createColumnHelper<
  Trainer & { profession?: string | null }
>();

const columns = [
  columnHelper.accessor("trainer_name", {
    header: "Trainer",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("profession", {
    header: "Profession",
    cell: (info) => {
      const val = info.getValue();
      return val ? <ProfessionBadge profession={val} /> : null;
    },
  }),
  columnHelper.accessor("ranks", {
    header: "Ranks",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("modified_ranks", {
    header: "Modified",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor(
    (row) => row.ranks + row.modified_ranks,
    {
      id: "total",
      header: "Total",
      cell: (info) => info.getValue(),
    },
  ),
  columnHelper.accessor("date_of_last_rank", {
    header: "Last Rank",
    cell: (info) => {
      const val = info.getValue();
      return val ? val.split(" ")[0] : "";
    },
  }),
];

const PROFESSION_ORDER = [
  "Fighter",
  "Healer",
  "Mystic",
  "Ranger",
  "Bloodmage",
  "Champion",
];

export function TrainersView() {
  const { trainers } = useStore();
  const [showZero, setShowZero] = useState(false);
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  const enrichedTrainers = useMemo(() => {
    // Build profession map from trainerDb
    const profMap = new Map<string, string | null>();
    for (const t of trainerDb) {
      profMap.set(t.name, t.profession);
    }

    // Map trainers with their character data, enriched with profession
    const charTrainerMap = new Map<string, Trainer>();
    for (const t of trainers) {
      charTrainerMap.set(t.trainer_name, t);
    }

    if (showZero) {
      // Merge all known trainers, showing zeros for untrained ones
      const allTrainers: (Trainer & { profession?: string | null })[] = [];
      for (const dbTrainer of trainerDb) {
        const existing = charTrainerMap.get(dbTrainer.name);
        if (existing) {
          allTrainers.push({
            ...existing,
            profession: dbTrainer.profession,
          });
        } else {
          allTrainers.push({
            id: null,
            character_id: 0,
            trainer_name: dbTrainer.name,
            ranks: 0,
            modified_ranks: 0,
            date_of_last_rank: null,
            profession: dbTrainer.profession,
          });
        }
      }
      return allTrainers;
    }

    return trainers.map((t) => ({
      ...t,
      profession: profMap.get(t.trainer_name) ?? null,
    }));
  }, [trainers, trainerDb, showZero]);

  // Group by profession
  const grouped = useMemo(() => {
    const groups = new Map<string, typeof enrichedTrainers>();
    for (const t of enrichedTrainers) {
      const prof = t.profession ?? "Other";
      if (!groups.has(prof)) groups.set(prof, []);
      groups.get(prof)!.push(t);
    }
    // Sort groups by profession order
    const ordered: [string, typeof enrichedTrainers][] = [];
    for (const p of PROFESSION_ORDER) {
      if (groups.has(p)) {
        ordered.push([p, groups.get(p)!]);
        groups.delete(p);
      }
    }
    // Remaining groups (Other, etc.)
    for (const [k, v] of groups) {
      ordered.push([k, v]);
    }
    return ordered;
  }, [enrichedTrainers]);

  const totalRanks = trainers.reduce(
    (s, t) => s + t.ranks + t.modified_ranks,
    0,
  );

  return (
    <div className="flex h-full flex-col">
      <div className="mb-4 flex items-center justify-between">
        <div className="text-sm text-[var(--color-text-muted)]">
          {trainers.length} trainers, {totalRanks.toLocaleString()} total ranks
        </div>
        <label className="flex cursor-pointer items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={showZero}
            onChange={(e) => setShowZero(e.target.checked)}
            className="accent-[var(--color-accent)]"
          />
          Show Zero Trainers
        </label>
      </div>

      {grouped.length === 0 ? (
        <div className="py-12 text-center text-[var(--color-text-muted)]">
          No trainer data
        </div>
      ) : (
        <div className="min-h-0 flex-1 space-y-4 overflow-auto">
          {grouped.map(([profession, groupTrainers]) => (
            <div key={profession}>
              <div className="mb-2 flex items-center gap-2">
                <ProfessionBadge profession={profession} />
                <span className="text-xs text-[var(--color-text-muted)]">
                  ({groupTrainers.length})
                </span>
              </div>
              <DataTable data={groupTrainers} columns={columns} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
