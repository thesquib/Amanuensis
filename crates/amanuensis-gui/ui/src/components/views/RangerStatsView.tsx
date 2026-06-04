import { useMemo, useState } from "react";
import { useStore } from "../../lib/store";
import { StatCard } from "../shared/StatCard";
import { computeRangerStats } from "../../lib/rangerStats";
import { getBestiaryMap } from "../../lib/bestiary";
import creatureValues from "../../../../data/creature_values.json";
import { StudiesPanel } from "./ranger/StudiesPanel";
import { FamiliesPanel } from "./ranger/FamiliesPanel";
import { TopTargetsPanel } from "./ranger/TopTargetsPanel";

const PANELS = [
  { id: "studies" as const, label: "Studies" },
  { id: "families" as const, label: "Families" },
  { id: "targets" as const, label: "Top Targets" },
];

// One-time nudge: befriend/morph tracking was added after some users had already
// scanned. Those databases hold movements-only studies until the logs are reparsed.
// Dismissal is persisted so the banner never nags again.
const BEFRIEND_MORPH_RESCAN_DISMISSED = "ranger-befriend-morph-rescan-dismissed";

export function RangerStatsView() {
  const { lastys, trainers, rangerStatsViewState, setRangerStatsViewState, selectedCharacterId, characters } = useStore();
  const character = characters.find((c) => c.id === selectedCharacterId);

  // Bestiary is loaded once at boot and never mutated, so a getState() snapshot
  // is safe here and intentionally non-reactive.
  const stats = useMemo(
    () => computeRangerStats(lastys, trainers, creatureValues, getBestiaryMap()),
    [lastys, trainers],
  );

  const activePanel = rangerStatsViewState.activePanel;

  const [rescanDismissed, setRescanDismissed] = useState(
    () => localStorage.getItem(BEFRIEND_MORPH_RESCAN_DISMISSED) === "1",
  );
  // Movements completed but no befriends/morphs recorded → likely a pre-feature
  // database that needs a rescan to populate the new tracking.
  const showRescanBanner =
    !rescanDismissed &&
    stats.total_studies > 0 &&
    stats.total_befriends === 0 &&
    stats.total_morphs === 0;
  const dismissRescanBanner = () => {
    localStorage.setItem(BEFRIEND_MORPH_RESCAN_DISMISSED, "1");
    setRescanDismissed(true);
  };

  return (
    <div>
      {showRescanBanner && (
        <div className="mb-4 flex items-center justify-between rounded border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
          <span>
            Befriend &amp; Morph tracking was recently added. If these counts look low, click{" "}
            <span className="font-semibold">Rescan Logs</span> in the sidebar to populate them from your existing logs.
          </span>
          <button
            type="button"
            onClick={dismissRescanBanner}
            className="ml-2 shrink-0 text-xs text-amber-400 hover:text-amber-200"
          >
            Dismiss
          </button>
        </div>
      )}

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
