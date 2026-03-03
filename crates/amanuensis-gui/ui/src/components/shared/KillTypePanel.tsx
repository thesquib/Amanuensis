import type { Kill } from "../../types";
import { CreatureImage } from "./CreatureImage";
import { timeAgo } from "../../lib/timeAgo";

interface KillTypePanelProps {
  label: string;
  highest: Kill | null;
  lowestRecent: Kill | null;
  dateField: keyof Kill;
}

export function KillTypePanel({ label, highest, lowestRecent, dateField }: KillTypePanelProps) {
  return (
    <div className="rounded-lg bg-[var(--color-card)] p-4">
      <div className="mb-3 text-xs uppercase tracking-wide text-[var(--color-text-muted)]">
        {label}
      </div>
      <div>
        <div className="mb-1 text-xs text-[var(--color-text-muted)]">Highest ever</div>
        {highest ? (
          <div className="flex items-center gap-2">
            <CreatureImage
              creatureName={highest.creature_name}
              className="h-10 w-auto flex-shrink-0"
            />
            <div>
              <div className="font-semibold leading-tight">{highest.creature_name}</div>
              <div className="text-xs text-[var(--color-text-muted)]">
                Value: {highest.creature_value}
              </div>
              {timeAgo(highest[dateField] as string | null) && (
                <div className="text-xs text-[var(--color-text-muted)]">
                  {timeAgo(highest[dateField] as string | null)}
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="text-sm text-[var(--color-text-muted)]">None</div>
        )}
      </div>
      <div className="mt-3 border-t border-[var(--color-border)] pt-3">
        <div className="mb-1 text-xs text-[var(--color-text-muted)]">Lowest (last 20)</div>
        {lowestRecent ? (
          <div className="flex items-center gap-2">
            <CreatureImage
              creatureName={lowestRecent.creature_name}
              className="h-8 w-auto flex-shrink-0"
            />
            <div>
              <div className="text-sm font-medium leading-tight">
                {lowestRecent.creature_name}
              </div>
              <div className="text-xs text-[var(--color-text-muted)]">
                Value: {lowestRecent.creature_value}
              </div>
              {timeAgo(lowestRecent[dateField] as string | null) && (
                <div className="text-xs text-[var(--color-text-muted)]">
                  {timeAgo(lowestRecent[dateField] as string | null)}
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="text-sm text-[var(--color-text-muted)]">None</div>
        )}
      </div>
    </div>
  );
}
