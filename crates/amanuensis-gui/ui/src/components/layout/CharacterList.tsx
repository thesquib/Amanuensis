import { useState } from "react";
import { useStore } from "../../lib/store";
import { ProfessionBadge } from "../shared/ProfessionBadge";

interface CharacterListProps {
  onSelectCharacter: (charId: number) => Promise<void>;
}

export function CharacterList({ onSelectCharacter }: CharacterListProps) {
  const {
    characters,
    selectedCharacterId,
    minRanks,
    setMinRanks,
    excludeUnknown,
    setExcludeUnknown,
    dbPath,
  } = useStore();

  const [search, setSearch] = useState("");
  const [ranksInput, setRanksInput] = useState(String(minRanks));

  const filtered = characters.filter((char) => {
    if (minRanks > 0 && char.total_ranks < minRanks) return false;
    if (excludeUnknown && (char.profession === "Unknown" || char.name.toLowerCase().startsWith("agratis"))) return false;
    if (search && !char.name.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  function handleRanksBlur() {
    const n = parseInt(ranksInput, 10);
    if (!isNaN(n) && n >= 0 && n <= 100) {
      setMinRanks(n);
    } else {
      setRanksInput(String(minRanks));
    }
  }

  return (
    <>
      {characters.length > 0 && (
        <div className="flex flex-col gap-1 border-b border-[var(--color-border)] px-2 py-2">
          <input
            type="text"
            placeholder="Search characters..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full rounded border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-1 text-xs text-[var(--color-text)] placeholder-[var(--color-text-muted)] outline-none focus:border-[var(--color-accent)]"
          />
          <div className="flex items-center gap-3">
            <label className="flex items-center gap-1 text-xs text-[var(--color-text-muted)]">
              Min ranks
              <input
                type="number"
                min={0}
                max={100}
                value={ranksInput}
                onChange={(e) => setRanksInput(e.target.value)}
                onBlur={handleRanksBlur}
                onKeyDown={(e) => e.key === "Enter" && handleRanksBlur()}
                className="w-12 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-1 py-0.5 text-center text-xs text-[var(--color-text)] outline-none focus:border-[var(--color-accent)]"
              />
            </label>
            <label className="flex items-center gap-1 text-xs text-[var(--color-text-muted)]">
              <input
                type="checkbox"
                checked={excludeUnknown}
                onChange={(e) => setExcludeUnknown(e.target.checked)}
                className="accent-[var(--color-accent)]"
              />
              Excl. Unknown
            </label>
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {filtered.map((char) => (
          <button
            key={char.id}
            onClick={() => char.id !== null && onSelectCharacter(char.id)}
            className={`flex w-full items-center gap-2 px-2 py-1.5 text-left text-sm hover:bg-[var(--color-card)]/30 ${
              selectedCharacterId === char.id ? "bg-[var(--color-card)]/50" : ""
            }`}
          >
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium leading-tight">{char.name}</div>
              <div className="flex items-center gap-1.5">
                <ProfessionBadge profession={char.profession} />
                <span className="text-[11px] text-[var(--color-text-muted)]">
                  {char.total_ranks > 0 ? `${char.total_ranks} ranks` : "0 ranks"}
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
    </>
  );
}
