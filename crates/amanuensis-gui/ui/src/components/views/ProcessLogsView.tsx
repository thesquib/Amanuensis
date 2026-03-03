import { useStore } from "../../lib/store";

const LEVEL_STYLES: Record<string, string> = {
  error: "bg-red-500/20 text-red-400",
  warn: "bg-yellow-500/20 text-yellow-400",
  info: "bg-blue-500/20 text-blue-400",
};

export function ProcessLogsView() {
  const { processLogs } = useStore();

  if (processLogs.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-[var(--color-text-muted)]">
        <div className="text-lg font-medium">No scan logs</div>
        <div className="mt-2 max-w-sm text-center text-sm">
          Warnings and errors from your last scan will appear here.
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="mb-3 text-xs text-[var(--color-text-muted)]">
        {processLogs.length} entr{processLogs.length === 1 ? "y" : "ies"} from last scan
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto space-y-1.5">
        {processLogs.map((log) => (
          <div
            key={log.id}
            className="flex items-start gap-2.5 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2"
          >
            <span
              className={`mt-0.5 shrink-0 rounded px-1.5 py-0.5 text-xs font-semibold uppercase ${LEVEL_STYLES[log.level] ?? "bg-[var(--color-border)] text-[var(--color-text-muted)]"}`}
            >
              {log.level}
            </span>
            <div className="min-w-0 flex-1">
              <div className="break-words text-sm leading-snug text-[var(--color-text)]">
                {log.message}
              </div>
              <div className="mt-0.5 text-xs text-[var(--color-text-muted)]">
                {log.created_at}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
