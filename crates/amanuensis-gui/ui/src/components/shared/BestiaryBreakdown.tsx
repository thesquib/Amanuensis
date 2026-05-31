import { useMemo } from "react";
import type { Kill } from "../../types";
import { useStore } from "../../lib/store";
import { normalizeBestiaryLabel } from "../../lib/bestiary";

interface BestiaryBreakdownProps {
  kills: Kill[];
}

interface AggRow {
  key: string;
  kills: number;
  pct: number;
}

function aggregate(kills: Kill[], group: (k: Kill) => string): AggRow[] {
  const counts = new Map<string, number>();
  let total = 0;
  for (const k of kills) {
    const totalForKill =
      k.killed_count +
      k.slaughtered_count +
      k.vanquished_count +
      k.dispatched_count +
      k.assisted_kill_count +
      k.assisted_slaughter_count +
      k.assisted_vanquish_count +
      k.assisted_dispatch_count;
    if (totalForKill === 0) continue;
    const key = group(k) || "Unknown";
    counts.set(key, (counts.get(key) ?? 0) + totalForKill);
    total += totalForKill;
  }
  return Array.from(counts.entries())
    .map(([key, count]) => ({
      key,
      kills: count,
      pct: total > 0 ? (count / total) * 100 : 0,
    }))
    .sort((a, b) => b.kills - a.kills || a.key.localeCompare(b.key));
}

export function BestiaryBreakdown({ kills }: BestiaryBreakdownProps) {
  const byName = useStore((s) => s.bestiaryByName);

  const byFamily = useMemo(
    () => aggregate(kills, (k) => byName[k.creature_name]?.family ?? ""),
    [kills, byName],
  );
  const byRarity = useMemo(
    () => aggregate(kills, (k) => normalizeBestiaryLabel(byName[k.creature_name]?.rarity ?? "")),
    [kills, byName],
  );

  if (byFamily.length === 0) return null;

  return (
    <section className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4">
      <h3 className="mb-3 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Bestiary breakdown
      </h3>
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <BreakdownTable title="By family" rows={byFamily} />
        <BreakdownTable title="By rarity" rows={byRarity} />
      </div>
    </section>
  );
}

function BreakdownTable({ title, rows }: { title: string; rows: AggRow[] }) {
  return (
    <div>
      <h4 className="mb-1 text-xs font-semibold text-[var(--color-text-muted)]">{title}</h4>
      <table className="w-full text-xs tabular-nums">
        <thead>
          <tr className="border-b border-[var(--color-border)] text-[var(--color-text-muted)]">
            <th className="py-1 pr-3 text-left">{title.replace("By ", "")}</th>
            <th className="py-1 pl-3 pr-4 text-right">Kills</th>
            <th className="w-14 py-1 pl-3 text-right">%</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.key} className="border-b border-[var(--color-border)]/40">
              <td className="py-1 pr-3">{r.key}</td>
              <td className="py-1 pl-3 pr-4 text-right">{r.kills.toLocaleString()}</td>
              <td className="w-14 py-1 pl-3 text-right">{Math.round(r.pct * 10) / 10}%</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
