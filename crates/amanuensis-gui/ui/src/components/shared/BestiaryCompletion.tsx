import { useEffect, useMemo, useState } from "react";
import { getEncounteredCreatures } from "../../lib/commands";
import { useStore } from "../../lib/store";

interface BestiaryCompletionProps {
  characterId: number;
}

interface FamilyRow {
  family: string;
  encountered: number;
  total: number;
  pct: number;
}

export function BestiaryCompletion({ characterId }: BestiaryCompletionProps) {
  const bestiary = useStore((s) => s.bestiary);
  const [encountered, setEncountered] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getEncounteredCreatures(characterId)
      .then((names) => {
        if (!cancelled) setEncountered(new Set(names));
      })
      .catch((err) => console.error("Failed to load encountered creatures", err));
    return () => {
      cancelled = true;
    };
  }, [characterId]);

  const total = bestiary.length;
  const encCount = useMemo(
    () => bestiary.reduce((acc, e) => acc + (encountered.has(e.name) ? 1 : 0), 0),
    [bestiary, encountered],
  );
  const pct = total > 0 ? Math.round((encCount / total) * 1000) / 10 : 0;

  const families: FamilyRow[] = useMemo(() => {
    const rows = new Map<string, { encountered: number; total: number }>();
    for (const entry of bestiary) {
      const fam = entry.family ?? "Unknown";
      const row = rows.get(fam) ?? { encountered: 0, total: 0 };
      row.total += 1;
      if (encountered.has(entry.name)) row.encountered += 1;
      rows.set(fam, row);
    }
    return Array.from(rows.entries())
      .map(([family, { encountered, total }]) => ({
        family,
        encountered,
        total,
        pct: total > 0 ? (encountered / total) * 100 : 0,
      }))
      .sort((a, b) => b.pct - a.pct || a.family.localeCompare(b.family));
  }, [bestiary, encountered]);

  if (total === 0) return null;

  return (
    <section className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-4">
      <h3 className="mb-2 text-sm font-semibold uppercase tracking-wide text-[var(--color-text-muted)]">
        Bestiary completion
      </h3>
      <div className="flex items-baseline gap-3">
        <div className="text-2xl font-bold">
          {encCount} / {total}
        </div>
        <div className="text-sm text-[var(--color-text-muted)]">{pct}% encountered</div>
      </div>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="mt-3 text-xs text-[var(--color-accent)] underline"
      >
        {open ? "Hide" : "Show"} per-family breakdown
      </button>
      {open && (
        <table className="mt-3 w-full text-xs tabular-nums">
          <thead>
            <tr className="border-b border-[var(--color-border)] text-[var(--color-text-muted)]">
              <th className="py-1 pr-3 text-left">Family</th>
              <th className="py-1 pl-3 pr-4 text-right">Encountered</th>
              <th className="py-1 pl-3 pr-4 text-right">Total</th>
              <th className="w-14 py-1 pl-3 text-right">%</th>
            </tr>
          </thead>
          <tbody>
            {families.map((r) => (
              <tr key={r.family} className="border-b border-[var(--color-border)]/40">
                <td className="py-1 pr-3">{r.family}</td>
                <td className="py-1 pl-3 pr-4 text-right">{r.encountered.toLocaleString()}</td>
                <td className="py-1 pl-3 pr-4 text-right">{r.total.toLocaleString()}</td>
                <td className="w-14 py-1 pl-3 text-right">{Math.round(r.pct * 10) / 10}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
