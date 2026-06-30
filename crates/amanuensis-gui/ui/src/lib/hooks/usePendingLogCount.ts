import { useEffect } from "react";
import { useStore } from "../store";
import { getPendingLogCount } from "../commands";

/**
 * Keeps the "Update Logs (N)" badge accurate WITHOUT a live filesystem watcher.
 * Recomputes the cheap, stat-based count of new/grown logs:
 *  - when the DB opens or the source list changes, and
 *  - whenever the window regains focus (e.g. switching back after a play session).
 * Never ingests data — it only counts.
 *
 * (A live FSEvents watcher was removed: macOS FSEvents silently fails to deliver events
 *  on external/USB volumes, where many players keep their logs. Stat-based counting works
 *  on every volume.)
 */
export function usePendingLogCount() {
  const dbPath = useStore((s) => s.dbPath);
  const sources = useStore((s) => s.sources);
  const setPendingLogCount = useStore((s) => s.setPendingLogCount);

  useEffect(() => {
    if (!dbPath || sources.length === 0) {
      setPendingLogCount(0);
      return;
    }
    let cancelled = false;
    const refresh = () => {
      getPendingLogCount(sources)
        .then((n) => {
          if (!cancelled) setPendingLogCount(n);
        })
        .catch((e) => console.error("pending count failed:", e));
    };
    refresh();
    window.addEventListener("focus", refresh);
    return () => {
      cancelled = true;
      window.removeEventListener("focus", refresh);
    };
  }, [dbPath, sources, setPendingLogCount]);
}
