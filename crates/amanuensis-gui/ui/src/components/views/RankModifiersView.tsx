import { useMemo, useState, useEffect, useCallback } from "react";
import { useStore } from "../../lib/store";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import {
  getTrainerDbInfo,
  getTrainers,
  listCharacters,
  setModifiedRanks,
} from "../../lib/commands";
import type { Trainer, TrainerInfo } from "../../types";

const PROFESSION_ORDER = [
  "Fighter",
  "Healer",
  "Mystic",
  "Ranger",
  "Bloodmage",
  "Champion",
];

interface TrainerRow {
  name: string;
  profession: string;
  ranks: number;
  modified_ranks: number;
  multiplier: number;
  is_combo: boolean;
  combo_components: string[];
}

function ModifiedRankInput({
  value,
  onSave,
}: {
  value: number;
  onSave: (val: number) => void;
}) {
  const [draft, setDraft] = useState(value === 0 ? "" : String(value));
  const [saving, setSaving] = useState(false);

  // Sync draft when external value changes
  useEffect(() => {
    setDraft(value === 0 ? "" : String(value));
  }, [value]);

  const commit = useCallback(() => {
    const parsed = draft.trim() === "" ? 0 : parseInt(draft, 10);
    if (isNaN(parsed)) {
      // Revert to current value
      setDraft(value === 0 ? "" : String(value));
      return;
    }
    if (parsed !== value) {
      setSaving(true);
      onSave(parsed);
      // saving indicator clears when value prop updates
      setTimeout(() => setSaving(false), 300);
    }
  }, [draft, value, onSave]);

  return (
    <input
      type="text"
      inputMode="numeric"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.currentTarget.blur();
        }
      }}
      placeholder="0"
      className={`w-20 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-1 text-right text-sm transition-colors focus:border-[var(--color-accent)] focus:outline-none ${
        saving ? "border-[var(--color-accent)]" : ""
      }`}
    />
  );
}

export function RankModifiersView() {
  const { trainers, setTrainers, setCharacters, selectedCharacterId } = useStore();
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  const toggleGroup = useCallback((profession: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(profession)) {
        next.delete(profession);
      } else {
        next.add(profession);
      }
      return next;
    });
  }, []);

  // Build merged trainer rows: all known trainers with character data overlaid
  const trainerRows = useMemo(() => {
    const charTrainerMap = new Map<string, Trainer>();
    for (const t of trainers) {
      charTrainerMap.set(t.trainer_name, t);
    }

    const rows: TrainerRow[] = [];
    for (const dbTrainer of trainerDb) {
      const existing = charTrainerMap.get(dbTrainer.name);
      rows.push({
        name: dbTrainer.name,
        profession: dbTrainer.profession ?? "Other",
        ranks: existing?.ranks ?? 0,
        modified_ranks: existing?.modified_ranks ?? 0,
        multiplier: dbTrainer.multiplier,
        is_combo: dbTrainer.is_combo,
        combo_components: dbTrainer.combo_components,
      });
    }
    return rows;
  }, [trainers, trainerDb]);

  // Filter by search query
  const filteredRows = useMemo(() => {
    if (!searchQuery.trim()) return trainerRows;
    const q = searchQuery.trim().toLowerCase();
    return trainerRows.filter((t) => t.name.toLowerCase().includes(q));
  }, [trainerRows, searchQuery]);

  // Group by profession in standard order
  const grouped = useMemo(() => {
    const groups = new Map<string, TrainerRow[]>();
    for (const t of filteredRows) {
      if (!groups.has(t.profession)) groups.set(t.profession, []);
      groups.get(t.profession)!.push(t);
    }
    const ordered: [string, TrainerRow[]][] = [];
    for (const p of PROFESSION_ORDER) {
      if (groups.has(p)) {
        ordered.push([p, groups.get(p)!]);
        groups.delete(p);
      }
    }
    for (const [k, v] of groups) {
      ordered.push([k, v]);
    }
    return ordered;
  }, [filteredRows]);

  const handleSave = useCallback(
    async (trainerName: string, value: number) => {
      if (selectedCharacterId == null) return;
      try {
        await setModifiedRanks(selectedCharacterId, trainerName, value);
        const [updated, chars] = await Promise.all([
          getTrainers(selectedCharacterId),
          listCharacters(),
        ]);
        setTrainers(updated);
        setCharacters(chars);
      } catch (e) {
        console.error("Failed to save modified ranks:", e);
      }
    },
    [selectedCharacterId, setTrainers, setCharacters],
  );

  // Summary stats (use unfiltered rows for totals)
  const totalTrainers = trainerRows.length;
  const logRanks = trainerRows.reduce((s, t) => s + t.ranks, 0);
  const modifiedRanks = trainerRows.reduce((s, t) => s + t.modified_ranks, 0);
  const totalRanks = logRanks + modifiedRanks;
  const effectiveTotal = trainerRows.reduce(
    (s, t) => s + (t.ranks + t.modified_ranks) * t.multiplier,
    0,
  );
  const effectiveRounded = Math.round(effectiveTotal * 10) / 10;

  return (
    <div className="flex h-full flex-col">
      <div className="mb-1 flex items-center justify-between">
        <div className="text-sm text-[var(--color-text-muted)]">
          {totalTrainers} trainers, {totalRanks.toLocaleString()} total ranks (
          {logRanks.toLocaleString()} from logs, {modifiedRanks.toLocaleString()}{" "}
          modified) | {effectiveRounded} effective
        </div>
      </div>
      <div className="mb-3 text-xs text-[var(--color-text-muted)]">
        Add ranks that don't appear in logs for whatever reason
      </div>

      <div className="mb-3">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search trainers..."
          className="w-full max-w-xs rounded border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-sm transition-colors placeholder:text-[var(--color-text-muted)] focus:border-[var(--color-accent)] focus:outline-none"
        />
        {searchQuery.trim() && (
          <span className="ml-2 text-xs text-[var(--color-text-muted)]">
            {filteredRows.length} matching
          </span>
        )}
      </div>

      {grouped.length === 0 ? (
        <div className="py-12 text-center text-[var(--color-text-muted)]">
          {searchQuery.trim() ? "No matching trainers" : "No trainer data"}
        </div>
      ) : (
        <div className="min-h-0 flex-1 space-y-4">
          {grouped.map(([profession, groupTrainers]) => {
            const isCollapsed = collapsedGroups.has(profession);
            return (
              <div key={profession}>
                <button
                  type="button"
                  onClick={() => toggleGroup(profession)}
                  className="mb-2 flex w-full items-center gap-2 text-left"
                >
                  <span className="text-xs text-[var(--color-text-muted)]">
                    {isCollapsed ? "▶" : "▼"}
                  </span>
                  <ProfessionBadge profession={profession} />
                  <span className="text-xs text-[var(--color-text-muted)]">
                    ({groupTrainers.length})
                  </span>
                </button>
                {!isCollapsed && (
                  <div className="overflow-hidden rounded-lg border border-[var(--color-border)]">
                    <table className="w-full text-sm">
                      <thead>
                        <tr className="border-b border-[var(--color-border)] bg-[var(--color-card)]">
                          <th className="px-3 py-2 text-left font-medium">
                            Trainer
                          </th>
                          <th className="w-24 px-3 py-2 text-right font-medium">
                            Log Ranks
                          </th>
                          <th className="w-28 px-3 py-2 text-right font-medium">
                            Modified
                          </th>
                          <th className="w-24 px-3 py-2 text-right font-medium">
                            Total
                          </th>
                          <th className="w-24 px-3 py-2 text-right font-medium">
                            Effective
                          </th>
                        </tr>
                      </thead>
                      <tbody>
                        {groupTrainers.map((t) => (
                          <tr
                            key={t.name}
                            className="border-b border-[var(--color-border)] last:border-b-0"
                          >
                            <td className="px-3 py-1.5">
                              {t.name}
                              {t.is_combo && (
                                <span
                                  className="ml-1 cursor-help text-[var(--color-accent)]"
                                  title={`Combo trainer: includes ${t.combo_components.join(", ")}`}
                                >
                                  *
                                </span>
                              )}
                            </td>
                            <td className="px-3 py-1.5 text-right text-[var(--color-text-muted)]">
                              {t.ranks}
                            </td>
                            <td className="px-3 py-1.5 text-right">
                              <ModifiedRankInput
                                value={t.modified_ranks}
                                onSave={(val) => handleSave(t.name, val)}
                              />
                            </td>
                            <td className="px-3 py-1.5 text-right font-medium">
                              {t.ranks + t.modified_ranks}
                            </td>
                            <td className="px-3 py-1.5 text-right text-[var(--color-text-muted)]">
                              {(() => {
                                const eff =
                                  Math.round(
                                    (t.ranks + t.modified_ranks) *
                                      t.multiplier *
                                      10,
                                  ) / 10;
                                return eff % 1 === 0 ? eff : eff.toFixed(1);
                              })()}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
