import type { UpdateResult } from "../../types";

interface UpdateResultDialogProps {
  result: UpdateResult;
  onClose: () => void;
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

export function UpdateResultDialog({ result, onClose }: UpdateResultDialogProps) {
  const { scan, perCharacter } = result;
  const nothing = scan.files_scanned === 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="mb-1 text-lg font-bold">Update complete</h2>

        {nothing ? (
          <p className="mb-4 text-sm text-[var(--color-text-muted)]">
            Already up to date — no new or updated logs were found.
          </p>
        ) : (
          <>
            <p className="mb-3 text-sm text-[var(--color-text-muted)]">
              Read {plural(scan.files_scanned, "file")}
              {scan.skipped > 0 ? ` · skipped ${scan.skipped}` : ""} ·{" "}
              {plural(scan.events_found, "event")} found
              {scan.errors > 0 ? ` · ${plural(scan.errors, "error")}` : ""}.
            </p>

            {perCharacter.length > 0 ? (
              <div className="mb-4 max-h-64 overflow-y-auto rounded border border-[var(--color-border)]">
                {perCharacter.map((d) => {
                  const parts: string[] = [];
                  if (d.loginsDelta > 0) parts.push(`+${plural(d.loginsDelta, "login")}`);
                  if (d.deathsDelta > 0) parts.push(`+${plural(d.deathsDelta, "death")}`);
                  return (
                    <div
                      key={d.name}
                      className="flex items-center justify-between border-b border-[var(--color-border)] px-3 py-2 text-sm last:border-b-0"
                    >
                      <span className="font-medium">{d.name}</span>
                      <span className="text-xs text-[var(--color-text-muted)]">
                        {parts.length > 0 ? parts.join(" · ") : "updated"}
                      </span>
                    </div>
                  );
                })}
              </div>
            ) : (
              <p className="mb-4 text-xs text-[var(--color-text-muted)]">
                Files were read, but no per-character login or death changes were detected.
              </p>
            )}
          </>
        )}

        <div className="flex justify-end">
          <button
            onClick={onClose}
            className="rounded bg-[var(--color-accent)] px-4 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
