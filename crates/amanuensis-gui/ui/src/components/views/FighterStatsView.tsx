import { useState, useEffect, useMemo } from "react";
import { useStore } from "../../lib/store";
import { StatCard } from "../shared/StatCard";
import { getTrainerDbInfo } from "../../lib/commands";
import { computeFighterStats } from "../../lib/fighterStats";
import type { TrainerInfo } from "../../types";

export function FighterStatsView() {
  const { trainers } = useStore();
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  const stats = useMemo(() => {
    const ranks = new Map<string, number>();
    for (const t of trainers) {
      const total = t.ranks + t.modified_ranks;
      ranks.set(t.trainer_name, (ranks.get(t.trainer_name) ?? 0) + total);
    }

    const multipliers = new Map<string, number>();
    for (const t of trainerDb) {
      multipliers.set(t.name, t.multiplier);
    }

    return computeFighterStats(ranks, multipliers);
  }, [trainers, trainerDb]);

  return (
    <div>
      <h2 className="mb-4 text-xl font-bold">Fighter Stats</h2>
      <p className="mb-4 text-xs text-[var(--color-text-muted)]">
        Based on Gorvin's Fighter Calculator — Human, Roguewood Club, no items
      </p>

      {/* Overview */}
      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-3">
        <StatCard
          label="Trained Ranks"
          value={stats.trainedRanks.toLocaleString()}
        />
        <StatCard
          label="Effective Ranks"
          value={stats.effectiveRanks.toLocaleString()}
        />
        <StatCard
          label="Est. Slaughter Points"
          value={stats.slaughterPoints.toLocaleString()}
        />
      </div>

      {/* Primary stats */}
      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Primary Stats
      </h3>
      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        <StatCard
          label="Health"
          value={stats.health.toLocaleString()}
        />
        <StatCard
          label="Balance"
          value={stats.balance.toLocaleString()}
        />
        <StatCard
          label="Spirit"
          value={stats.spirit.toLocaleString()}
        />
        <StatCard
          label="Accuracy"
          value={stats.accuracy.toLocaleString()}
        />
        <StatCard
          label="Defense"
          value={stats.defense.toLocaleString()}
        />
        <StatCard
          label="Min Damage"
          value={stats.minDamage.toLocaleString()}
        />
        <StatCard
          label="Max Damage"
          value={stats.maxDamage.toLocaleString()}
        />
      </div>

      {/* Regen stats */}
      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Regeneration
      </h3>
      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        <StatCard
          label="Health Regen"
          value={stats.healthRegen.toLocaleString()}
        />
        <StatCard
          label="Balance Regen"
          value={stats.balRegen.toLocaleString()}
        />
        <StatCard
          label="Spirit Regen"
          value={stats.spiritRegen.toLocaleString()}
        />
        <StatCard
          label="Heal Receptivity"
          value={stats.healReceptivity.toLocaleString()}
        />
      </div>

      {/* Derived stats */}
      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Derived Stats
      </h3>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        <StatCard
          label="Damage Range"
          value={`${stats.damageMin} – ${stats.damageMax}`}
        />
        <StatCard
          label="Balance / Swing"
          value={stats.balancePerSwing.toLocaleString()}
        />
        <StatCard
          label="Shieldstone Drain"
          value={stats.shieldstoneDrain.toLocaleString()}
          sub="per use"
        />
        <StatCard
          label="Health / Frame"
          value={stats.healthPerFrame.toFixed(2)}
        />
        <StatCard
          label="Balance / Frame"
          value={stats.balancePerFrame.toFixed(1)}
        />
        <StatCard
          label="Spirit / Frame"
          value={stats.spiritPerFrame.toFixed(2)}
        />
      </div>
    </div>
  );
}
