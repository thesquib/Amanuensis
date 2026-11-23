import { useEffect, useState } from "react";
import { acknowledgeTrainerData, getTrainerDataStatus, type TrainerDataStatus } from "../../lib/commands";
import { useStore } from "../../lib/store";

export function TrainerDataBanner() {
  const dbPath = useStore((s) => s.dbPath);
  const isScanning = useStore((s) => s.isScanning);
  const setActiveView = useStore((s) => s.setActiveView);
  const [status, setStatus] = useState<TrainerDataStatus | null>(null);

  // Re-read after each scan: a rescan clears the stale flag.
  useEffect(() => {
    if (!dbPath || isScanning) return;
    getTrainerDataStatus().then(setStatus).catch(() => setStatus(null));
  }, [dbPath, isScanning]);

  if (!status || !status.show) return null;

  const dismiss = () => {
    acknowledgeTrainerData()
      .then(() => setStatus({ ...status, acknowledged: true, show: false }))
      .catch((e) => console.error("acknowledge failed:", e));
  };

  return (
    <div className="border-b border-[var(--color-border)] bg-[var(--color-accent)]/10 px-4 py-2 text-sm">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="font-medium">{status.title}</div>
          <div>{status.body}</div>
        </div>
        <button
          onClick={dismiss}
          className="shrink-0 text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
          aria-label="Dismiss"
        >
          Dismiss
        </button>
      </div>
      {status.modifiers_to_review.length > 0 && (
        <div className="mt-2">
          <ul className="list-disc pl-5">
            {status.modifiers_to_review.map((i) => (
              <li key={`${i.character}/${i.trainer}`}>{i.reason}</li>
            ))}
          </ul>
          <button
            onClick={() => setActiveView("rank-modifiers")}
            className="mt-1 font-medium text-[var(--color-accent)] underline hover:no-underline"
          >
            Open Rank Modifiers
          </button>
        </div>
      )}
    </div>
  );
}
