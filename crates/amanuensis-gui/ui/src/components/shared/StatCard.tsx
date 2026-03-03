import type { ReactNode } from "react";

interface StatCardProps {
  label: string;
  value: string | number;
  sub?: string;
  image?: ReactNode;
  large?: boolean;
  compact?: boolean;
  className?: string;
}

export function StatCard({ label, value, sub, image, large, compact, className }: StatCardProps) {
  if (compact) {
    return (
      <div className={`rounded-lg bg-[var(--color-card)] px-3 py-2 flex items-center justify-between gap-2 ${className ?? ""}`}>
        <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">
          {label}
        </div>
        <div className="flex items-center gap-1.5 min-w-0">
          {image}
          <div className="text-right min-w-0">
            <div className="text-sm font-semibold truncate">{value}</div>
            {sub && (
              <div className="text-xs text-[var(--color-text-muted)] truncate">{sub}</div>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={`rounded-lg bg-[var(--color-card)] p-4 ${className ?? ""}`}>
      <div className="min-h-8 text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
        {label}
      </div>
      <div className="mt-1 flex items-center gap-3">
        {image}
        <div>
          <div className={large ? "text-4xl font-bold" : "text-2xl font-bold"}>{value}</div>
          {sub && (
            <div className="mt-0.5 text-xs text-[var(--color-text-muted)]">
              {sub}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
