import { useState, useMemo } from "react";
import { createColumnHelper, type ColumnDef } from "@tanstack/react-table";
import { DataTable } from "../../shared/DataTable";
import { CreatureImage } from "../../shared/CreatureImage";
import type { MorphCandidate, FamilyProgress } from "../../../lib/rangerStats";

function StagePill({ stage }: { stage: string }) {
  const colors: Record<string, string> = {
    Befriend: "bg-blue-500/15 text-blue-400",
    Movements: "bg-purple-500/15 text-purple-400",
    Studying: "bg-yellow-500/15 text-yellow-400",
    None: "bg-[var(--color-border)] text-[var(--color-text-muted)]",
  };
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${colors[stage] ?? colors.None}`}>
      {stage}
    </span>
  );
}

type TargetCategory = "all" | "atkus" | "swings" | "higgrus" | "defense" | "darkus";

const CATEGORIES: { id: TargetCategory; label: string; description: string }[] = [
  { id: "all", label: "All", description: "Sorted by creature value" },
  { id: "atkus", label: "Top Atkus", description: "Highest attack — boost accuracy" },
  { id: "swings", label: "Top Swings", description: "Fastest swingers — lowest frames/swing" },
  { id: "higgrus", label: "Top Higgrus", description: "Highest health — boost health damage" },
  { id: "defense", label: "Top Defense", description: "Highest defense" },
  { id: "darkus", label: "Top Darkus", description: "Highest damage — boost damage absorption" },
];

const candidateHelper = createColumnHelper<MorphCandidate>();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function buildColumns(category: TargetCategory): ColumnDef<MorphCandidate, any>[] {
  const cols: ColumnDef<MorphCandidate, any>[] = [
    candidateHelper.accessor("creature_name", {
      header: "Creature",
      cell: (info) => (
        <div className="flex items-center gap-2">
          <CreatureImage creatureName={info.getValue()} className="h-6 w-6" />
          <span>{info.getValue()}</span>
        </div>
      ),
    }),
    candidateHelper.accessor("family", {
      header: "Family",
      cell: (info) => info.getValue() || "—",
    }),
    candidateHelper.accessor("value", {
      header: "Value",
      cell: (info) => info.getValue().toLocaleString(),
    }),
  ];

  // Add the stat column relevant to the category
  if (category === "atkus") {
    cols.push(
      candidateHelper.accessor("atk", {
        header: "Attack",
        cell: (info) => info.getValue() > 0 ? info.getValue().toLocaleString() : "—",
      }),
    );
  } else if (category === "swings") {
    cols.push(
      candidateHelper.accessor("fps", {
        header: "Frames/Swing",
        cell: (info) => info.getValue() > 0 ? info.getValue().toString() : "—",
      }),
    );
  } else if (category === "higgrus") {
    cols.push(
      candidateHelper.accessor("hp", {
        header: "Health",
        cell: (info) => info.getValue() > 0 ? info.getValue().toLocaleString() : "—",
      }),
    );
  } else if (category === "defense") {
    cols.push(
      candidateHelper.accessor("def", {
        header: "Defense",
        cell: (info) => info.getValue() > 0 ? info.getValue().toLocaleString() : "—",
      }),
    );
  } else if (category === "darkus") {
    cols.push(
      candidateHelper.accessor("dmg", {
        header: "Damage",
        cell: (info) => info.getValue() > 0 ? info.getValue().toLocaleString() : "—",
      }),
    );
  }

  cols.push(
    candidateHelper.accessor("current_stage", {
      header: "Stage",
      cell: (info) => <StagePill stage={info.getValue()} />,
    }),
    candidateHelper.accessor("duvin_remaining", {
      header: "Duvin Needed",
      cell: (info) => info.getValue(),
    }),
  );

  return cols;
}

function sortCandidates(candidates: MorphCandidate[], category: TargetCategory): MorphCandidate[] {
  const sorted = [...candidates];
  switch (category) {
    case "atkus":
      sorted.sort((a, b) => b.atk - a.atk || b.value - a.value);
      return sorted.filter((c) => c.atk > 0).slice(0, 20);
    case "swings":
      // Lower fps = faster = better, filter out 0 (unknown)
      sorted.sort((a, b) => {
        if (a.fps <= 0 && b.fps <= 0) return b.value - a.value;
        if (a.fps <= 0) return 1;
        if (b.fps <= 0) return -1;
        return a.fps - b.fps || b.value - a.value;
      });
      return sorted.filter((c) => c.fps > 0).slice(0, 20);
    case "higgrus":
      sorted.sort((a, b) => b.hp - a.hp || b.value - a.value);
      return sorted.filter((c) => c.hp > 0).slice(0, 20);
    case "defense":
      sorted.sort((a, b) => b.def - a.def || b.value - a.value);
      return sorted.filter((c) => c.def > 0).slice(0, 20);
    case "darkus":
      sorted.sort((a, b) => b.dmg - a.dmg || b.value - a.value);
      return sorted.filter((c) => c.dmg > 0).slice(0, 20);
    default:
      return sorted.slice(0, 20);
  }
}

interface TopTargetsPanelProps {
  morph_candidates: MorphCandidate[];
  families: FamilyProgress[];
  coinLevel: number;
}

export function TopTargetsPanel({ morph_candidates, families, coinLevel }: TopTargetsPanelProps) {
  const [category, setCategory] = useState<TargetCategory>("all");
  const [maxValue, setMaxValue] = useState<number | null>(null);

  // Default to coin level; user can override
  const effectiveMax = maxValue ?? coinLevel;

  const activeCategory = CATEGORIES.find((c) => c.id === category)!;
  const columns = useMemo(() => buildColumns(category), [category]);
  const filtered = useMemo(
    () => effectiveMax > 0
      ? morph_candidates.filter((c) => c.value <= effectiveMax)
      : morph_candidates,
    [morph_candidates, effectiveMax],
  );
  const sorted = useMemo(() => sortCandidates(filtered, category), [filtered, category]);

  // Families sorted by proximity to 50% bonus (closest to maxing first, excluding already maxed)
  const familyCompletion = families
    .filter((f) => !f.is_maxed && f.movements_completed > 0)
    .sort((a, b) => b.bonus_pct - a.bonus_pct || a.family.localeCompare(b.family));

  return (
    <div className="space-y-8">
      {/* Category tabs + max value filter */}
      <div>
        <div className="mb-2 flex flex-wrap items-center gap-1">
          {CATEGORIES.map((cat) => (
            <button
              key={cat.id}
              onClick={() => setCategory(cat.id)}
              className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                category === cat.id
                  ? "bg-[var(--color-accent)] text-white"
                  : "bg-[var(--color-card)] text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
              }`}
            >
              {cat.label}
            </button>
          ))}
          <div className="ml-auto flex items-center gap-2">
            <label className="text-xs text-[var(--color-text-muted)]">Max value</label>
            <input
              type="number"
              value={effectiveMax}
              onChange={(e) => {
                const v = parseInt(e.target.value, 10);
                setMaxValue(Number.isNaN(v) ? 0 : v);
              }}
              min={0}
              className="w-24 rounded border border-[var(--color-border)] bg-[var(--color-sidebar)] px-2 py-1 text-right text-sm text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
            />
            {maxValue !== null && maxValue !== coinLevel && (
              <button
                onClick={() => setMaxValue(null)}
                className="text-xs text-[var(--color-accent)] hover:underline"
              >
                Reset
              </button>
            )}
          </div>
        </div>
        <p className="mb-3 text-xs text-[var(--color-text-muted)]">
          {activeCategory.description}
          {effectiveMax > 0 && <span> — showing creatures with value ≤ {effectiveMax.toLocaleString()}</span>}
        </p>

        {sorted.length > 0 ? (
          <DataTable data={sorted} columns={columns} />
        ) : (
          <div className="py-8 text-center text-[var(--color-text-muted)]">
            No eligible creatures found
          </div>
        )}
      </div>

      {/* Family Completion Progress */}
      {familyCompletion.length > 0 && (
        <div>
          <h3 className="mb-3 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
            Family Completion Progress
          </h3>
          <div className="overflow-auto">
            <table className="w-full border-collapse text-sm">
              <thead>
                <tr>
                  <th className="border-b border-[var(--color-border)] px-3 py-2 text-left text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
                    Family
                  </th>
                  <th className="border-b border-[var(--color-border)] px-3 py-2 text-left text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
                    Movements
                  </th>
                  <th className="border-b border-[var(--color-border)] px-3 py-2 text-left text-xs font-medium uppercase tracking-wide text-[var(--color-text-muted)]">
                    Progress
                  </th>
                </tr>
              </thead>
              <tbody>
                {familyCompletion.map((fp) => (
                  <tr key={fp.family} className="border-b border-[var(--color-border)] hover:bg-[var(--color-card)]/30">
                    <td className="px-3 py-1.5 font-medium">{fp.family}</td>
                    <td className="px-3 py-1.5">{fp.movements_completed} / 5</td>
                    <td className="px-3 py-1.5">
                      <div className="flex items-center gap-2">
                        <div className="h-1.5 w-24 overflow-hidden rounded-full bg-[var(--color-border)]">
                          <div
                            className="h-full rounded-full bg-[var(--color-accent)] transition-all"
                            style={{ width: `${(fp.bonus_pct / 50) * 100}%` }}
                          />
                        </div>
                        <span className="text-xs text-[var(--color-text-muted)]">{fp.bonus_pct}%</span>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
