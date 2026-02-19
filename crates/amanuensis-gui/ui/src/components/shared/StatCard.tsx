interface StatCardProps {
  label: string;
  value: string | number;
  sub?: string;
}

export function StatCard({ label, value, sub }: StatCardProps) {
  return (
    <div className="rounded-lg bg-[var(--color-card)] p-4">
      <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
        {label}
      </div>
      <div className="mt-1 text-2xl font-bold">{value}</div>
      {sub && (
        <div className="mt-0.5 text-xs text-[var(--color-text-muted)]">
          {sub}
        </div>
      )}
    </div>
  );
}
