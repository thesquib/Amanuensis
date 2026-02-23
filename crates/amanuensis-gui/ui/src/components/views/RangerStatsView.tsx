import { useMemo } from "react";
import { useStore } from "../../lib/store";
import { StatCard } from "../shared/StatCard";
import { computeRangerStats } from "../../lib/rangerStats";
import { bestiaryMap } from "../../lib/bestiary";
import creatureValues from "../../../../data/creature_values.json";
import { StudiesPanel } from "./ranger/StudiesPanel";
import { FamiliesPanel } from "./ranger/FamiliesPanel";
import { TopTargetsPanel } from "./ranger/TopTargetsPanel";

const PANELS = [
  { id: "studies" as const, label: "Studies" },
  { id: "families" as const, label: "Families" },
  { id: "targets" as const, label: "Top Targets" },
];

export function RangerStatsView() {
  const { lastys, trainers, rangerStatsViewState, setRangerStatsViewState, selectedCharacterId, characters } = useStore();
  const character = characters.find((c) => c.id === selectedCharacterId);

  const stats = useMemo(
    () => computeRangerStats(lastys, trainers, creatureValues, bestiaryMap),
    [lastys, trainers],
  );

  const activePanel = rangerStatsViewState.activePanel;

  return (
    <div>
      {/* Summary cards */}
      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-8">
        <StatCard label="Gossamer" value={stats.gossamer_ranks} />
        <StatCard label="Duvin Beastlore" value={stats.duvin_ranks} />
        <StatCard label="Total Duvin" value={stats.total_duvin} />
        <StatCard
          label="Available"
          value={Math.max(0, stats.duvin_available)}
          sub={stats.duvin_available < 0 ? "(data may be incomplete)" : undefined}
        />
        <StatCard label="Spent" value={stats.duvin_spent} />
        <StatCard label="Studies" value={stats.total_studies} />
        <StatCard label="Befriends" value={stats.total_befriends} />
        <StatCard label="Morphs" value={stats.total_morphs} />
      </div>

      {/* Panel tabs */}
      <div className="mb-4 flex gap-1 rounded-lg bg-[var(--color-sidebar)] p-1">
        {PANELS.map((panel) => (
          <button
            key={panel.id}
            onClick={() => setRangerStatsViewState({ activePanel: panel.id })}
            className={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${
              activePanel === panel.id
                ? "bg-[var(--color-card)] text-[var(--color-text)]"
                : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
            }`}
          >
            {panel.label}
          </button>
        ))}
      </div>

      {/* Active panel */}
      {activePanel === "studies" && <StudiesPanel studies={stats.studies} />}
      {activePanel === "families" && <FamiliesPanel families={stats.families} />}
      {activePanel === "targets" && (
        <TopTargetsPanel morph_candidates={stats.morph_candidates} families={stats.families} coinLevel={character?.coin_level ?? 0} />
      )}
    </div>
  );
}
