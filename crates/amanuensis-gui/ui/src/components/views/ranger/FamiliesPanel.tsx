import { CreatureImage } from "../../shared/CreatureImage";
import type { FamilyProgress } from "../../../lib/rangerStats";

interface FamiliesPanelProps {
  families: FamilyProgress[];
}

export function FamiliesPanel({ families }: FamiliesPanelProps) {
  if (families.length === 0) {
    return (
      <div className="py-12 text-center text-[var(--color-text-muted)]">
        No family progress yet
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
      {families.map((fp) => (
        <div
          key={fp.family}
          className="rounded-lg bg-[var(--color-card)] p-4"
        >
          <div className="mb-2 flex items-center gap-2">
            <CreatureImage creatureName={fp.representative_creature} className="h-8 w-8" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate text-sm font-semibold">{fp.family}</span>
                {fp.is_maxed && (
                  <span className="shrink-0 rounded-full bg-green-500/15 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-green-400">
                    Maxed
                  </span>
                )}
              </div>
            </div>
          </div>

          <div className="mb-2 space-y-1 text-xs text-[var(--color-text-muted)]">
            <div className="flex justify-between">
              <span>Movements</span>
              <span className="font-medium text-[var(--color-text)]">{fp.movements_completed}</span>
            </div>
            <div className="flex justify-between">
              <span>Befriends</span>
              <span className="font-medium text-[var(--color-text)]">{fp.befriends_completed}</span>
            </div>
            <div className="flex justify-between">
              <span>Morphs</span>
              <span className="font-medium text-[var(--color-text)]">{fp.morphs_completed}</span>
            </div>
          </div>

          {/* Bonus progress bar */}
          <div className="mt-2">
            <div className="mb-1 flex items-center justify-between text-[10px] uppercase tracking-wide text-[var(--color-text-muted)]">
              <span>Gossamer Bonus</span>
              <span className="font-medium text-[var(--color-text)]">{fp.bonus_pct}%</span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-[var(--color-border)]">
              <div
                className="h-full rounded-full bg-[var(--color-accent)] transition-all"
                style={{ width: `${(fp.bonus_pct / 50) * 100}%` }}
              />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
