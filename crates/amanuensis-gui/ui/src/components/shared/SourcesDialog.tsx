import { useStore } from "../../lib/store";

interface SourcesDialogProps {
  onClose: () => void;
  onRescan: () => void;
  isScanning: boolean;
}

export function SourcesDialog({ onClose, onRescan, isScanning }: SourcesDialogProps) {
  const { sources, removeSource } = useStore();

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="mb-1 text-lg font-bold">Log Sources</h2>
        <p className="mb-4 text-xs text-[var(--color-text-muted)]">
          Folders scanned into this app. Rescan replays all of them. Removing a source only
          forgets the folder; it does not delete already-scanned data until the next rescan.
        </p>

        {sources.length === 0 ? (
          <div className="mb-4 rounded border border-[var(--color-border)] px-3 py-6 text-center text-sm text-[var(--color-text-muted)]">
            No log folders yet — use Scan Log Folder(s) to add one.
          </div>
        ) : (
          <div className="mb-4 max-h-64 overflow-y-auto rounded border border-[var(--color-border)]">
            {sources.map((src) => (
              <div
                key={src.path}
                className="flex items-center gap-3 border-b border-[var(--color-border)] px-3 py-2 last:border-b-0"
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm" title={src.path}>
                    {src.path}
                  </div>
                  {src.recursive && (
                    <span className="text-xs text-[var(--color-text-muted)]">deep scan</span>
                  )}
                </div>
                <button
                  type="button"
                  onClick={() => removeSource(src.path)}
                  disabled={isScanning}
                  className="shrink-0 text-xs text-red-400 hover:text-red-300 disabled:opacity-50"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        )}

        <div className="flex justify-between">
          <button
            type="button"
            onClick={onRescan}
            disabled={isScanning || sources.length === 0}
            className="rounded bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80 disabled:opacity-50"
          >
            {isScanning ? "Rescanning..." : "Rescan all"}
          </button>
          <button
            type="button"
            onClick={onClose}
            className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
