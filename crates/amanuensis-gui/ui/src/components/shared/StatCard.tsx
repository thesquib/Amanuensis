import type { ReactNode } from "react";

interface StatCardProps {
  label: string;
  value: string | number;
  sub?: string;
  image?: ReactNode;
}

export function StatCard({ label, value, sub, image }: StatCardProps) {
  return (
    <div className="rounded-lg bg-[var(--color-card)] p-4">
      <div className="min-h-8 text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
        {label}
      </div>
      <div className="mt-1 flex items-center gap-3">
        {image}
        <div>
          <div className="text-2xl font-bold">{value}</div>
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
