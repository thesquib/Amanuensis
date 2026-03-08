import { useState } from "react";
import { useStore } from "../../lib/store";
import { listCharacters } from "../../lib/commands";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import { MergeDialog } from "../shared/MergeDialog";

interface CharacterListProps {
  onSelectCharacter: (charId: number) => Promise<void>;
}

export function CharacterList({ onSelectCharacter }: CharacterListProps) {
  const {
    characters,
    setCharacters,
    selectedCharacterId,
    isScanning,
    excludeLowCL,
    setExcludeLowCL,
    excludeUnknown,
    setExcludeUnknown,
    dbPath,
    coinLevelByCharId,
  } = useStore();

  const [showMergeDialog, setShowMergeDialog] = useState(false);

  const filtered = characters.filter((char) => {
    // Filter uses DB value — avoids characters disappearing when clicked and TS recomputes
    if (excludeLowCL && Math.max(char.coin_level, char.coin_level_interim) < 1) return false;
    if (excludeUnknown && char.profession === "Unknown") return false;
    return true;
  });

  return (
    <>
      {characters.length > 0 && (
        <div className="flex flex-col gap-1 border-b border-[var(--color-border)] px-3 py-2">
          <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
            <input
              type="checkbox"
              checked={excludeLowCL}
              onChange={(e) => setExcludeLowCL(e.target.checked)}
              className="accent-[var(--color-accent)]"
            />
            Exclude Lvl &lt; 1
          </label>
          <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
            <input
              type="checkbox"
              checked={excludeUnknown}
              onChange={(e) => setExcludeUnknown(e.target.checked)}
              className="accent-[var(--color-accent)]"
            />
            Exclude Unknown
          </label>
          {characters.length >= 2 && (
            <button
              onClick={() => setShowMergeDialog(true)}
              disabled={isScanning}
              className="mt-1 rounded border border-[var(--color-border)] px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)] disabled:opacity-50"
            >
              Merge Characters...
            </button>
          )}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {filtered.map((char) => (
          <button
            key={char.id}
            onClick={() => char.id !== null && onSelectCharacter(char.id)}
            className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--color-card)]/30 ${
              selectedCharacterId === char.id ? "bg-[var(--color-card)]/50" : ""
            }`}
          >
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{char.name}</div>
              <div className="flex items-center gap-2">
                <ProfessionBadge profession={char.profession} />
                <span className="text-xs text-[var(--color-text-muted)]">
                  {(() => {
                    const tsCL = char.id !== null ? coinLevelByCharId[char.id] : undefined;
                    if (tsCL !== undefined) {
                      return tsCL > 0 ? `Lvl ${tsCL}` : "Lvl 0";
                    }
                    // Fallback while character data hasn't been loaded yet
                    const cl = char.coin_level;
                    const interim = char.coin_level_interim;
                    const display = Math.max(cl, interim);
                    const estimated = cl === 0 && interim > 0;
                    return display > 0
                      ? `Lvl ${display}${estimated ? "*" : ""}`
                      : "Lvl 0";
                  })()}
                </span>
              </div>
            </div>
          </button>
        ))}
        {characters.length === 0 && dbPath && (
          <div className="p-3 text-center text-xs text-[var(--color-text-muted)]">
            No characters found.
            <br />
            Scan logs to get started.
          </div>
        )}
        {!dbPath && (
          <div className="p-3 text-center text-xs text-[var(--color-text-muted)]">
            Scan logs to get started.
          </div>
        )}
      </div>

      {showMergeDialog && (
        <MergeDialog
          characters={characters}
          onClose={() => setShowMergeDialog(false)}
          onMerged={async () => {
            setShowMergeDialog(false);
            const chars = await listCharacters();
            setCharacters(chars);
            if (chars.length > 0 && chars[0].id !== null) {
              await onSelectCharacter(chars[0].id);
            }
          }}
        />
      )}
    </>
  );
}
