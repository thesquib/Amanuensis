import { useMemo, useState, useEffect, useCallback } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import { getTrainerDbInfo } from "../../lib/commands";
import type { Trainer, TrainerInfo } from "../../types";

type EnrichedTrainer = Trainer & {
  profession?: string | null;
  multiplier: number;
  is_combo: boolean;
  combo_components: string[];
};

const columnHelper = createColumnHelper<EnrichedTrainer>();

const PROFESSION_ORDER = [
  "Fighter",
  "Healer",
  "Mystic",
  "Ranger",
  "Bloodmage",
  "Champion",
];

export function TrainersView() {
  const { trainers, trainersViewState, setTrainersViewState } = useStore();
  const { showZero, showEffective, searchQuery, collapsedGroups: collapsedArr } = trainersViewState;
  const collapsedGroups = useMemo(() => new Set(collapsedArr), [collapsedArr]);
  const setShowZero = useCallback((v: boolean) => setTrainersViewState({ showZero: v }), [setTrainersViewState]);
  const setShowEffective = useCallback((v: boolean) => setTrainersViewState({ showEffective: v }), [setTrainersViewState]);
  const setSearchQuery = useCallback((v: string) => setTrainersViewState({ searchQuery: v }), [setTrainersViewState]);
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  const toggleGroup = useCallback((profession: string) => {
    const next = new Set(collapsedGroups);
    if (next.has(profession)) {
      next.delete(profession);
    } else {
      next.add(profession);
    }
    setTrainersViewState({ collapsedGroups: [...next] });
  }, [collapsedGroups, setTrainersViewState]);

  const columns = useMemo(
    () => [
      columnHelper.accessor("trainer_name", {
        header: "Trainer",
        cell: (info) => {
          const row = info.row.original;
          if (showEffective && row.is_combo) {
            return (
              <span className="flex items-center gap-1">
                {info.getValue()}
                <span
                  className="cursor-help text-[var(--color-accent)]"
                  title={`Combo trainer: includes ${row.combo_components.join(", ")}`}
                >
                  *
                </span>
              </span>
            );
          }
          return info.getValue();
        },
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
      columnHelper.accessor("apply_learning_ranks", {
        id: "applied",
        header: "Applied",
        cell: (info) => {
          const row = info.row.original;
          const confirmed = row.apply_learning_ranks;
          const unknown = row.apply_learning_unknown_count;
          if (confirmed === 0 && unknown === 0) return 0;
          if (unknown > 0) {
            return (
              <span>
                {confirmed}
                <span
                  className="text-[var(--color-text-muted)]"
                  title={`${unknown} partial apply-learning event${unknown > 1 ? "s" : ""} (1-9 ranks each, exact amount unknown)`}
                >
                  +{unknown}?
                </span>
              </span>
            );
          }
          return confirmed;
        },
      }),
      columnHelper.accessor(
        (row) => row.ranks + row.modified_ranks + row.apply_learning_ranks,
        {
          id: "total",
          header: "Total",
          cell: (info) => info.getValue(),
        },
      ),
      ...(showEffective
        ? [
            columnHelper.accessor(
              (row: EnrichedTrainer) =>
                Math.round(
                  (row.ranks + row.modified_ranks + row.apply_learning_ranks) * row.multiplier * 10,
                ) / 10,
              {
                id: "effective",
                header: "Effective",
                cell: (info: { getValue: () => number }) => {
                  const val = info.getValue();
                  return val % 1 === 0 ? val : val.toFixed(1);
                },
              },
            ),
          ]
        : []),
      columnHelper.accessor("date_of_last_rank", {
        header: "Last Rank",
        cell: (info) => {
          const val = info.getValue();
          return val ? val.split(" ")[0] : "";
        },
      }),
    ],
    [showEffective],
  );

  const enrichedTrainers = useMemo(() => {
    // Build metadata maps from trainerDb
    const metaMap = new Map<
      string,
      { profession: string | null; multiplier: number; is_combo: boolean; combo_components: string[] }
    >();
    for (const t of trainerDb) {
      metaMap.set(t.name, {
        profession: t.profession,
        multiplier: t.multiplier,
        is_combo: t.is_combo,
        combo_components: t.combo_components,
      });
    }

    const charTrainerMap = new Map<string, Trainer>();
    for (const t of trainers) {
      charTrainerMap.set(t.trainer_name, t);
    }

    const defaultMeta = { multiplier: 1.0, is_combo: false, combo_components: [] as string[] };

    if (showZero) {
      const allTrainers: EnrichedTrainer[] = [];
      for (const dbTrainer of trainerDb) {
        const existing = charTrainerMap.get(dbTrainer.name);
        if (existing) {
          allTrainers.push({
            ...existing,
            profession: dbTrainer.profession,
            multiplier: dbTrainer.multiplier,
            is_combo: dbTrainer.is_combo,
            combo_components: dbTrainer.combo_components,
          });
        } else {
          allTrainers.push({
            id: null,
            character_id: 0,
            trainer_name: dbTrainer.name,
            ranks: 0,
            modified_ranks: 0,
            date_of_last_rank: null,
            apply_learning_ranks: 0,
            apply_learning_unknown_count: 0,
            profession: dbTrainer.profession,
            multiplier: dbTrainer.multiplier,
            is_combo: dbTrainer.is_combo,
            combo_components: dbTrainer.combo_components,
          });
        }
      }
      return allTrainers;
    }

    return trainers.map((t) => {
      const meta = metaMap.get(t.trainer_name);
      return {
        ...t,
        profession: meta?.profession ?? null,
        multiplier: meta?.multiplier ?? defaultMeta.multiplier,
        is_combo: meta?.is_combo ?? defaultMeta.is_combo,
        combo_components: meta?.combo_components ?? defaultMeta.combo_components,
      };
    });
  }, [trainers, trainerDb, showZero]);

  // Filter by search query
  const filteredTrainers = useMemo(() => {
    if (!searchQuery.trim()) return enrichedTrainers;
    const q = searchQuery.trim().toLowerCase();
    return enrichedTrainers.filter((t) => t.trainer_name.toLowerCase().includes(q));
  }, [enrichedTrainers, searchQuery]);

  // Group by profession
  const grouped = useMemo(() => {
    const groups = new Map<string, typeof filteredTrainers>();
    for (const t of filteredTrainers) {
      const prof = t.profession ?? "Other";
      if (!groups.has(prof)) groups.set(prof, []);
      groups.get(prof)!.push(t);
    }
    const ordered: [string, typeof filteredTrainers][] = [];
    for (const p of PROFESSION_ORDER) {
      if (groups.has(p)) {
        ordered.push([p, groups.get(p)!]);
        groups.delete(p);
      }
    }
    for (const [k, v] of groups) {
      ordered.push([k, v]);
    }
    return ordered;
  }, [filteredTrainers]);

  const totalRanks = trainers.reduce(
    (s, t) => s + t.ranks + t.modified_ranks + t.apply_learning_ranks,
    0,
  );

  const effectiveTotal = useMemo(() => {
    return enrichedTrainers.reduce(
      (s, t) => s + (t.ranks + t.modified_ranks + t.apply_learning_ranks) * t.multiplier,
      0,
    );
  }, [enrichedTrainers]);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-4 flex items-center justify-between">
        <div className="text-sm text-[var(--color-text-muted)]">
          {trainers.length} trainers, {totalRanks.toLocaleString()} total ranks
          {showEffective && (
            <span>
              {" "}
              ({Math.round(effectiveTotal * 10) / 10} effective)
            </span>
          )}
        </div>
        <div className="flex items-center gap-4">
          <label className="flex cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={showEffective}
              onChange={(e) => setShowEffective(e.target.checked)}
              className="accent-[var(--color-accent)]"
            />
            Show Effective Ranks
          </label>
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
      </div>

      <div className="mb-3">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search trainers..."
          className="w-full max-w-xs rounded border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-sm transition-colors placeholder:text-[var(--color-text-muted)] focus:border-[var(--color-accent)] focus:outline-none"
        />
        {searchQuery.trim() && (
          <span className="ml-2 text-xs text-[var(--color-text-muted)]">
            {filteredTrainers.length} matching
          </span>
        )}
      </div>

      {grouped.length === 0 ? (
        <div className="py-12 text-center text-[var(--color-text-muted)]">
          {searchQuery.trim() ? "No matching trainers" : "No trainer data"}
        </div>
      ) : (
        <div className="min-h-0 flex-1 space-y-4">
          {grouped.map(([profession, groupTrainers]) => {
            const isCollapsed = collapsedGroups.has(profession);
            return (
              <div key={profession}>
                <button
                  type="button"
                  onClick={() => toggleGroup(profession)}
                  className="mb-2 flex w-full items-center gap-2 text-left"
                >
                  <span className="text-xs text-[var(--color-text-muted)]">
                    {isCollapsed ? "▶" : "▼"}
                  </span>
                  <ProfessionBadge profession={profession} />
                  <span className="text-xs text-[var(--color-text-muted)]">
                    ({groupTrainers.length})
                  </span>
                </button>
                {!isCollapsed && (
                  <DataTable data={groupTrainers} columns={columns} />
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
