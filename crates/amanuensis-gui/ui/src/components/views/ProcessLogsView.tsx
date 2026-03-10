import { useState } from "react";
import { useStore } from "../../lib/store";

const LEVEL_STYLES: Record<string, string> = {
  error: "bg-red-500/20 text-red-400",
  warn: "bg-yellow-500/20 text-yellow-400",
  info: "bg-blue-500/20 text-blue-400",
};

export function ProcessLogsView() {
  const { processLogs, warnsDismissed, setWarnsDismissed } = useStore();
  const [copied, setCopied] = useState(false);

  const warnCount = processLogs.filter((l) => l.level === "warn").length;
  const errorCount = processLogs.filter((l) => l.level === "error").length;

  const handleCopyAll = async () => {
    const text = processLogs
      .map((l) => `[${l.level.toUpperCase()}] ${l.created_at}  ${l.message}`)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard unavailable (can happen on some Linux WebKitGTK configurations)
    }
  };

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
      <div className="mb-3 flex items-center gap-3">
        <span className="text-xs text-[var(--color-text-muted)]">
          {processLogs.length} entr{processLogs.length === 1 ? "y" : "ies"} from last scan
        </span>
        {errorCount > 0 && (
          <span className="text-xs text-red-400">{errorCount} error(s)</span>
        )}
        {warnCount > 0 && (
          <span className="text-xs text-yellow-400">{warnCount} warning(s)</span>
        )}
        <div className="ml-auto flex items-center gap-2">
          {warnCount > 0 && !warnsDismissed && (
            <button
              onClick={() => setWarnsDismissed(true)}
              className="rounded border border-[var(--color-border)] px-2 py-0.5 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]"
              title="Dismiss warning badge on the Logs tab"
            >
              Clear Warnings
            </button>
          )}
          <button
            onClick={handleCopyAll}
            className="rounded border border-[var(--color-border)] px-2 py-0.5 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]"
          >
            {copied ? "Copied!" : "Copy All"}
          </button>
        </div>
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
