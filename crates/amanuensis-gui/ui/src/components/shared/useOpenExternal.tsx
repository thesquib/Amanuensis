import { useCallback, useState } from "react";
import { openExternal } from "../../lib/commands";

/**
 * Opens links in the user's browser and *shows it when that fails*.
 *
 * Launching a browser is the one thing the app does that depends entirely on the host
 * desktop, and it does fail in the wild — an AppImage's library paths poisoning `xdg-open`,
 * or no `xdg-utils` installed at all. Swallowing the rejection makes those look like a dead
 * menu item, so the failure gets a notice with the URL in it, which the user can at least
 * copy by hand.
 *
 * Usage:
 *   const { open, element } = useOpenExternal();
 *   <button onClick={() => open(url)}>…</button>
 *   {element}
 */
export function useOpenExternal() {
  const [failed, setFailed] = useState<{ url: string; reason: string } | null>(null);

  const open = useCallback((url: string) => {
    openExternal(url).catch((e) => setFailed({ url, reason: String(e) }));
  }, []);

  const element = failed ? (
    <div
      role="alert"
      className="fixed bottom-4 right-4 z-[60] max-w-sm rounded-md border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-2 text-xs shadow-lg"
    >
      <div className="mb-1 font-medium text-[var(--color-danger)]">
        Couldn't open your browser
      </div>
      <div className="mb-1 text-[var(--color-text-muted)]">{failed.reason}</div>
      <div className="mb-2 break-all select-text">{failed.url}</div>
      <div className="flex gap-3">
        <button
          className="text-[var(--color-accent)] hover:underline"
          onClick={() => {
            navigator.clipboard.writeText(failed.url).catch(() => {});
            setFailed(null);
          }}
        >
          Copy link
        </button>
        <button
          className="text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
          onClick={() => setFailed(null)}
        >
          Dismiss
        </button>
      </div>
    </div>
  ) : null;

  return { open, element };
}
