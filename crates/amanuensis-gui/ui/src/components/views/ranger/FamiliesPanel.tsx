import { CreatureImage } from "../../shared/CreatureImage";
import { useCreatureContextMenu } from "../../shared/useCreatureContextMenu";
import type { FamilyProgress } from "../../../lib/rangerStats";

interface FamiliesPanelProps {
  families: FamilyProgress[];
}

function FamilyCard({
  fp,
  onCreatureContextMenu,
}: {
  fp: FamilyProgress;
  onCreatureContextMenu: (e: React.MouseEvent, name: string) => void;
}) {
  return (
    // The card's representative creature stands in for the family here: it gives the same
    // two options as everywhere else, and its family page is this family's page.
    <div
      className={`cursor-context-menu rounded-lg p-4 ${fp.has_progress ? "bg-[var(--color-card)]" : "bg-[var(--color-card)] opacity-50"}`}
      onContextMenu={(e) => onCreatureContextMenu(e, fp.representative_creature)}
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
  );
}

export function FamiliesPanel({ families }: FamiliesPanelProps) {
  // Must run before the early return below — hooks cannot be called conditionally.
  const creatureMenu = useCreatureContextMenu();

  if (families.length === 0) {
    return (
      <div className="py-12 text-center text-[var(--color-text-muted)]">
        No family progress yet
      </div>
    );
  }

  const studied = families.filter((fp) => fp.has_progress);
  const unstudied = families.filter((fp) => !fp.has_progress);

  return (
    <div>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        {studied.map((fp) => (
          <FamilyCard key={fp.family} fp={fp} onCreatureContextMenu={creatureMenu.openFor} />
        ))}
      </div>

      {unstudied.length > 0 && (
        <>
          <div className="my-4 flex items-center gap-3">
            <div className="h-px flex-1 bg-[var(--color-border)]" />
            <span className="text-xs uppercase tracking-wider text-[var(--color-text-muted)]">
              Not yet studied
            </span>
            <div className="h-px flex-1 bg-[var(--color-border)]" />
          </div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
            {unstudied.map((fp) => (
              <FamilyCard key={fp.family} fp={fp} onCreatureContextMenu={creatureMenu.openFor} />
            ))}
          </div>
        </>
      )}
      {creatureMenu.element}
    </div>
  );
}
