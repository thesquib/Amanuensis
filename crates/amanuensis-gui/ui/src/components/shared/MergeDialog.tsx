import { useState, useCallback } from "react";
import { mergeCharacters } from "../../lib/commands";
import { ProfessionBadge } from "./ProfessionBadge";
import type { Character } from "../../types";

interface MergeDialogProps {
  characters: Character[];
  onClose: () => void;
  onMerged: () => void;
}

export function MergeDialog({ characters, onClose, onMerged }: MergeDialogProps) {
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [primaryId, setPrimaryId] = useState<number | null>(null);
  const [isMerging, setIsMerging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const toggleCharacter = useCallback((id: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
        // If we removed the primary, clear it
        if (primaryId === id) setPrimaryId(null);
      } else {
        next.add(id);
        // Auto-select primary if this is the first selection
        if (next.size === 1) setPrimaryId(id);
      }
      return next;
    });
  }, [primaryId]);

  const handleMerge = useCallback(async () => {
    if (!primaryId || selected.size < 2) return;
    const sourceIds = [...selected].filter((id) => id !== primaryId);
    setIsMerging(true);
    setError(null);
    try {
      await mergeCharacters(sourceIds, primaryId);
      onMerged();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsMerging(false);
    }
  }, [selected, primaryId, onMerged]);

  const canMerge = selected.size >= 2 && primaryId !== null && selected.has(primaryId);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-5 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="mb-1 text-lg font-bold">Merge Characters</h2>
        <p className="mb-4 text-xs text-[var(--color-text-muted)]">
          Select 2+ characters to merge, then pick which name to keep as the primary.
          The merge can be undone at any time.
        </p>

        <div className="mb-4 max-h-64 overflow-y-auto rounded border border-[var(--color-border)]">
          {characters.map((char) => {
            const id = char.id!;
            const isSelected = selected.has(id);
            const isPrimary = primaryId === id;
            return (
              <div
                key={id}
                className={`flex items-center gap-3 border-b border-[var(--color-border)] px-3 py-2 last:border-b-0 ${
                  isSelected ? "bg-[var(--color-card)]/50" : ""
                }`}
              >
                <input
                  type="checkbox"
                  checked={isSelected}
                  onChange={() => toggleCharacter(id)}
                  className="accent-[var(--color-accent)]"
                />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium">{char.name}</span>
                    <ProfessionBadge profession={char.profession} />
                    <span className="text-xs text-[var(--color-text-muted)]">
                      Lvl {char.coin_level}
                    </span>
                  </div>
                </div>
                {isSelected && selected.size >= 2 && (
                  <label className="flex items-center gap-1 text-xs text-[var(--color-text-muted)]">
                    <input
                      type="radio"
                      name="primary"
                      checked={isPrimary}
                      onChange={() => setPrimaryId(id)}
                      className="accent-[var(--color-accent)]"
                    />
                    Primary
                  </label>
                )}
              </div>
            );
          })}
        </div>

        {error && (
          <div className="mb-3 rounded bg-[var(--color-danger-bg)] px-3 py-2 text-xs text-[var(--color-danger)]">
            {error}
          </div>
        )}

        <div className="flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded border border-[var(--color-border)] px-4 py-1.5 text-sm hover:bg-[var(--color-card)]"
          >
            Cancel
          </button>
          <button
            onClick={handleMerge}
            disabled={!canMerge || isMerging}
            className="rounded bg-[var(--color-accent)] px-4 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80 disabled:opacity-50"
          >
            {isMerging ? "Merging..." : "Merge"}
          </button>
        </div>
      </div>
    </div>
  );
}
