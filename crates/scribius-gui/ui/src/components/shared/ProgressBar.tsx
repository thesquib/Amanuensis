interface ProgressBarProps {
  current: number;
  total: number;
  label?: string;
}

export function ProgressBar({ current, total, label }: ProgressBarProps) {
  const pct = total > 0 ? Math.round((current / total) * 100) : 0;
  return (
    <div className="w-full">
      {label && (
        <div className="mb-1 text-sm text-[var(--color-text-muted)]">
          {label}
        </div>
      )}
      <div className="h-2 w-full overflow-hidden rounded-full bg-[var(--color-border)]">
        <div
          className="h-full rounded-full bg-[var(--color-accent)] transition-[width] duration-150"
          style={{ width: `${pct}%` }}
        />
      </div>
      <div className="mt-1 text-xs text-[var(--color-text-muted)]">
        {current} / {total} ({pct}%)
      </div>
    </div>
  );
}
