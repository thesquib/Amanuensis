import { useStore } from "../../lib/store";

const OPTIONS = [
  { value: "all", label: "All data" },
  { value: "last_scan", label: "Last scan" },
] as const;

export function ScopeToggle() {
  const dataScope = useStore((s) => s.dataScope);
  const setDataScope = useStore((s) => s.setDataScope);
  return (
    <div className="inline-flex overflow-hidden rounded border border-[var(--color-border)] text-sm">
      {OPTIONS.map((o) => (
        <button
          key={o.value}
          type="button"
          onClick={() => setDataScope(o.value)}
          className={`px-3 py-1 transition-colors ${
            dataScope === o.value
              ? "bg-[var(--color-accent)] text-white"
              : "text-[var(--color-text-muted)] hover:bg-[var(--color-bg-hover)]"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export const LAST_SCAN_EMPTY = "Nothing found by the last scan yet. Run Update Logs.";
