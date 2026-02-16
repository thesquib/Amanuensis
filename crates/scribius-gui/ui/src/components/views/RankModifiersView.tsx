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

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
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
      });
    }
    return rows;
  }, [trainers, trainerDb]);

  // Group by profession in standard order
  const grouped = useMemo(() => {
    const groups = new Map<string, TrainerRow[]>();
    for (const t of trainerRows) {
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
  }, [trainerRows]);

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

  // Summary stats
  const totalTrainers = trainerRows.length;
  const logRanks = trainerRows.reduce((s, t) => s + t.ranks, 0);
  const modifiedRanks = trainerRows.reduce((s, t) => s + t.modified_ranks, 0);
  const totalRanks = logRanks + modifiedRanks;

  return (
    <div className="flex h-full flex-col">
      <div className="mb-1 flex items-center justify-between">
        <div className="text-sm text-[var(--color-text-muted)]">
          {totalTrainers} trainers, {totalRanks.toLocaleString()} total ranks (
          {logRanks.toLocaleString()} from logs, {modifiedRanks.toLocaleString()}{" "}
          modified)
        </div>
      </div>
      <div className="mb-4 text-xs text-[var(--color-text-muted)]">
        Add ranks that don't appear in logs for whatever reason
      </div>

      {grouped.length === 0 ? (
        <div className="py-12 text-center text-[var(--color-text-muted)]">
          No trainer data
        </div>
      ) : (
        <div className="min-h-0 flex-1 space-y-4 overflow-auto">
          {grouped.map(([profession, groupTrainers]) => (
            <div key={profession}>
              <div className="mb-2 flex items-center gap-2">
                <ProfessionBadge profession={profession} />
                <span className="text-xs text-[var(--color-text-muted)]">
                  ({groupTrainers.length})
                </span>
              </div>
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
                    </tr>
                  </thead>
                  <tbody>
                    {groupTrainers.map((t) => (
                      <tr
                        key={t.name}
                        className="border-b border-[var(--color-border)] last:border-b-0"
                      >
                        <td className="px-3 py-1.5">{t.name}</td>
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
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
