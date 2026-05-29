import { useMemo } from "react";
import type { Kill } from "../../types";
import { useStore } from "../../lib/store";

export interface KillsFilterState {
  families: Set<string>;
  rarities: Set<string>;
  seasonal: boolean;
}

interface KillsFilterBarProps {
  kills: Kill[];
  value: KillsFilterState;
  onChange: (next: KillsFilterState) => void;
}

export function KillsFilterBar({ kills, value, onChange }: KillsFilterBarProps) {
  const byName = useStore((s) => s.bestiaryByName);

  const { families, rarities } = useMemo(() => {
    const fam = new Set<string>();
    const rar = new Set<string>();
    for (const k of kills) {
      const e = byName[k.creature_name];
      if (e?.family) fam.add(e.family);
      if (e?.rarity) rar.add(e.rarity);
    }
    return {
      families: Array.from(fam).sort(),
      rarities: Array.from(rar).sort(),
    };
  }, [kills, byName]);

  const toggle = (set: Set<string>, key: string): Set<string> => {
    const next = new Set(set);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    return next;
  };

  return (
    <div className="mb-3 flex flex-wrap items-center gap-2 text-xs">
      <span className="text-[var(--color-text-muted)]">Family:</span>
      {families.map((f) => (
        <Chip
          key={f}
          label={f}
          active={value.families.has(f)}
          onClick={() => onChange({ ...value, families: toggle(value.families, f) })}
        />
      ))}
      <span className="ml-3 text-[var(--color-text-muted)]">Rarity:</span>
      {rarities.map((r) => (
        <Chip
          key={r}
          label={r}
          active={value.rarities.has(r)}
          onClick={() => onChange({ ...value, rarities: toggle(value.rarities, r) })}
        />
      ))}
      <Chip
        label="Seasonal"
        active={value.seasonal}
        onClick={() => onChange({ ...value, seasonal: !value.seasonal })}
      />
      {(value.families.size > 0 || value.rarities.size > 0 || value.seasonal) && (
        <button
          type="button"
          className="ml-2 text-[var(--color-accent)] underline"
          onClick={() =>
            onChange({ families: new Set(), rarities: new Set(), seasonal: false })
          }
        >
          Clear
        </button>
      )}
    </div>
  );
}

function Chip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full border px-2 py-0.5 transition ${
        active
          ? "border-[var(--color-accent)] bg-[var(--color-accent)]/15 text-[var(--color-accent)]"
          : "border-[var(--color-border)] text-[var(--color-text-muted)] hover:bg-[var(--color-bg-hover)]"
      }`}
    >
      {label}
    </button>
  );
}
