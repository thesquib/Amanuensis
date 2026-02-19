import { useState, useCallback } from "react";
import { useStore } from "../../lib/store";
import { searchLogs } from "../../lib/commands";
import type { LogSearchResult } from "../../types";

export function LogSearchView() {
  const { selectedCharacterId, logLineCount } = useStore();

  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<"character" | "all">("character");
  const [results, setResults] = useState<LogSearchResult[]>([]);
  const [resultCount, setResultCount] = useState<number | null>(null);
  const [isSearching, setIsSearching] = useState(false);

  const handleSearch = useCallback(async () => {
    const trimmed = query.trim();
    if (!trimmed) return;

    setIsSearching(true);
    try {
      const charId = scope === "character" ? selectedCharacterId : null;
      const res = await searchLogs(trimmed, charId, 200);
      setResults(res);
      setResultCount(res.length);
    } catch (e) {
      console.error("Search failed:", e);
      setResults([]);
      setResultCount(0);
    } finally {
      setIsSearching(false);
    }
  }, [query, scope, selectedCharacterId]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        handleSearch();
      }
    },
    [handleSearch],
  );

  if (logLineCount === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-[var(--color-text-muted)]">
        <div className="text-lg font-medium">No log lines indexed</div>
        <div className="mt-2 max-w-sm text-center text-sm">
          Make sure "Index logs for search" is checked in the sidebar, then
          rescan your logs to enable full-text search.
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      {/* Search bar */}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search log lines..."
          className="flex-1 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-sm text-[var(--color-text)] placeholder-[var(--color-text-muted)] outline-none focus:border-[var(--color-accent)]"
        />
        <select
          value={scope}
          onChange={(e) => setScope(e.target.value as "character" | "all")}
          className="rounded border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-2 text-sm text-[var(--color-text)] outline-none"
        >
          <option value="character">This Character</option>
          <option value="all">All Characters</option>
        </select>
        <button
          onClick={handleSearch}
          disabled={isSearching || !query.trim()}
          className="rounded bg-[var(--color-accent)] px-4 py-2 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80 disabled:opacity-50"
        >
          {isSearching ? "Searching..." : "Search"}
        </button>
      </div>

      {/* Result count */}
      {resultCount !== null && (
        <div className="mt-2 text-xs text-[var(--color-text-muted)]">
          {resultCount === 0
            ? "No matching log lines found"
            : `${resultCount} result${resultCount === 1 ? "" : "s"} found${resultCount >= 200 ? " (showing first 200)" : ""}`}
        </div>
      )}

      {/* Results */}
      <div className="mt-3 min-h-0 flex-1 overflow-y-auto">
        {resultCount === null && (
          <div className="py-8 text-center text-sm text-[var(--color-text-muted)]">
            Enter a search term and press Enter
          </div>
        )}
        {results.map((r, i) => (
          <div
            key={i}
            className="mb-2 rounded border border-[var(--color-border)] bg-[var(--color-card)] p-3"
          >
            <div className="mb-1.5 flex items-center gap-2 text-xs text-[var(--color-text-muted)]">
              <span className="rounded bg-[var(--color-accent)]/20 px-1.5 py-0.5 font-medium text-[var(--color-accent)]">
                {r.character_name}
              </span>
              {r.timestamp && <span>{r.timestamp}</span>}
              <span className="truncate" title={r.file_path}>
                {r.file_path.split("/").pop()}
              </span>
            </div>
            <div
              className="text-sm leading-relaxed [&_mark]:rounded [&_mark]:bg-yellow-500/30 [&_mark]:px-0.5 [&_mark]:text-[var(--color-text)]"
              dangerouslySetInnerHTML={{ __html: r.snippet }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
