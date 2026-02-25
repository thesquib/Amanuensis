import { useMemo, useState, useEffect, useCallback } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../lib/store";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import {
  clearRankOverrides,
  getTrainerDbInfo,
  getTrainers,
  listCharacters,
  setRankOverride,
} from "../../lib/commands";
import type { Trainer, TrainerInfo } from "../../types";

const PROFESSION_ORDER = [
  "Fighter",
  "Healer",
  "Mystic",
  "Ranger",
  "Bloodmage",
  "Champion",
  "Language",
  "Arts",
  "Trades",
];

type RankMode = "modifier" | "override" | "override_until_date";

interface TrainerRow {
  name: string;
  profession: string;
  ranks: number;
  modified_ranks: number;
  apply_learning_ranks: number;
  multiplier: number;
  is_combo: boolean;
  combo_components: string[];
  rank_mode: RankMode;
  override_date: string | null;
}

function todayMDYY(): string {
  const now = new Date();
  const m = now.getMonth() + 1;
  const d = now.getDate();
  const yy = String(now.getFullYear()).slice(-2);
  return `${m}/${d}/${yy}`;
}

const MODE_LABELS: Record<RankMode, string> = {
  modifier: "Modifier",
  override: "Override",
  override_until_date: "Override Until Date",
};

function RankModeInput({
  row,
  onSave,
}: {
  row: TrainerRow;
  onSave: (mode: RankMode, value: number, date: string | null) => void;
}) {
  const [mode, setMode] = useState<RankMode>(row.rank_mode);
  const [draft, setDraft] = useState(row.modified_ranks === 0 ? "" : String(row.modified_ranks));
  const [dateDraft, setDateDraft] = useState(row.override_date ?? "");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setMode(row.rank_mode);
    setDraft(row.modified_ranks === 0 ? "" : String(row.modified_ranks));
    setDateDraft(row.override_date ?? "");
  }, [row.rank_mode, row.modified_ranks, row.override_date]);

  const commit = useCallback(() => {
    const parsed = draft.trim() === "" ? 0 : parseInt(draft, 10);
    if (isNaN(parsed)) {
      setDraft(row.modified_ranks === 0 ? "" : String(row.modified_ranks));
      return;
    }
    const date = mode === "override_until_date" ? (dateDraft.trim() || null) : null;
    if (parsed !== row.modified_ranks || mode !== row.rank_mode || date !== row.override_date) {
      setSaving(true);
      onSave(mode, parsed, date);
      setTimeout(() => setSaving(false), 300);
    }
  }, [draft, dateDraft, mode, row, onSave]);

  const handleModeChange = useCallback((newMode: RankMode) => {
    setMode(newMode);
    // Auto-commit on mode change if value is set
    const parsed = draft.trim() === "" ? 0 : parseInt(draft, 10);
    if (isNaN(parsed)) return;
    // Default to today's date in M/D/YY format when switching to override_until_date
    let date: string | null = null;
    if (newMode === "override_until_date") {
      const effectiveDate = dateDraft.trim() || todayMDYY();
      setDateDraft(effectiveDate);
      date = effectiveDate;
    }
    setSaving(true);
    onSave(newMode, parsed, date);
    setTimeout(() => setSaving(false), 300);
  }, [draft, dateDraft, onSave]);

  const inputLabel = mode === "override" ? "Total Ranks" : mode === "override_until_date" ? "Baseline" : "Modifier";

  return (
    <div className="flex items-center gap-2">
      <select
        value={mode}
        onChange={(e) => handleModeChange(e.target.value as RankMode)}
        className={`rounded border border-[var(--color-border)] bg-[var(--color-card)] px-1.5 py-1 text-xs transition-colors focus:border-[var(--color-accent)] focus:outline-none ${
          saving ? "border-[var(--color-accent)]" : ""
        }`}
      >
        {(Object.keys(MODE_LABELS) as RankMode[]).map((m) => (
          <option key={m} value={m}>{MODE_LABELS[m]}</option>
        ))}
      </select>
      <input
        type="text"
        inputMode="numeric"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
        placeholder="0"
        title={inputLabel}
        className={`w-20 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-1 text-right text-sm transition-colors focus:border-[var(--color-accent)] focus:outline-none ${
          saving ? "border-[var(--color-accent)]" : ""
        }`}
      />
      {mode === "override_until_date" && (
        <input
          type="text"
          value={dateDraft}
          onChange={(e) => setDateDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); }}
          placeholder="M/D/YY"
          title="Cutoff date (ranks after this date are counted from logs)"
          className={`w-24 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-1 text-sm transition-colors focus:border-[var(--color-accent)] focus:outline-none ${
            saving ? "border-[var(--color-accent)]" : ""
          }`}
        />
      )}
    </div>
  );
}

function computeEffective(row: TrainerRow): number {
  switch (row.rank_mode) {
    case "override":
      return row.modified_ranks;
    case "override_until_date":
      return row.modified_ranks + row.ranks + row.apply_learning_ranks;
    default:
      return row.ranks + row.modified_ranks + row.apply_learning_ranks;
  }
}

export function RankModifiersView() {
  const { trainers, setTrainers, setCharacters, selectedCharacterId, rankModifiersViewState, setRankModifiersViewState } = useStore();
  const { searchQuery, collapsedGroups: collapsedArr } = rankModifiersViewState;
  const collapsedGroups = useMemo(() => new Set(collapsedArr), [collapsedArr]);
  const setSearchQuery = useCallback((v: string) => setRankModifiersViewState({ searchQuery: v }), [setRankModifiersViewState]);
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);
  const [rescanBanner, setRescanBanner] = useState(false);
  const hasSavedState = useMemo(() => localStorage.getItem("amanuensis_collapsed_rankModifiers") !== null, []);
  const [defaultsInitialized, setDefaultsInitialized] = useState(hasSavedState);

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  const toggleGroup = useCallback((profession: string) => {
    const next = new Set(collapsedGroups);
    if (next.has(profession)) {
      next.delete(profession);
    } else {
      next.add(profession);
    }
    setRankModifiersViewState({ collapsedGroups: [...next] });
  }, [collapsedGroups, setRankModifiersViewState]);

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
        apply_learning_ranks: existing?.apply_learning_ranks ?? 0,
        multiplier: dbTrainer.multiplier,
        is_combo: dbTrainer.is_combo,
        combo_components: dbTrainer.combo_components,
        rank_mode: existing?.rank_mode ?? "modifier",
        override_date: existing?.override_date ?? null,
      });
    }
    return rows;
  }, [trainers, trainerDb]);

  // Initialize default collapsed state: all groups collapsed except those with modifiers
  useEffect(() => {
    if (defaultsInitialized || trainerRows.length === 0) return;
    setDefaultsInitialized(true);

    // Find which professions have any modified ranks set
    const groupsWithModifiers = new Set<string>();
    for (const row of trainerRows) {
      if (row.modified_ranks !== 0) {
        groupsWithModifiers.add(row.profession);
      }
    }

    // Collect all professions and collapse those without modifiers
    const allProfessions = new Set<string>();
    for (const row of trainerRows) {
      allProfessions.add(row.profession);
    }

    const collapsed = [...allProfessions].filter((p) => !groupsWithModifiers.has(p));
    setRankModifiersViewState({ collapsedGroups: collapsed });
  }, [trainerRows, defaultsInitialized, setRankModifiersViewState]);

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
    async (trainerName: string, mode: RankMode, value: number, date: string | null) => {
      if (selectedCharacterId == null) return;
      try {
        await setRankOverride(selectedCharacterId, trainerName, mode, value, date);
        const [updated, chars] = await Promise.all([
          getTrainers(selectedCharacterId),
          listCharacters(),
        ]);
        setTrainers(updated);
        setCharacters(chars);
        if (mode !== "modifier") {
          setRescanBanner(true);
        }
      } catch (e) {
        console.error("Failed to save rank override:", e);
      }
    },
    [selectedCharacterId, setTrainers, setCharacters],
  );

  // Summary stats (use unfiltered rows for totals)
  const totalTrainers = trainerRows.length;
  const logRanks = trainerRows.reduce((s, t) => s + t.ranks, 0);
  const modifiedRanks = trainerRows.reduce((s, t) => s + t.modified_ranks, 0);
  const totalEffective = trainerRows.reduce((s, t) => s + computeEffective(t), 0);
  const weightedEffective = trainerRows.reduce(
    (s, t) => s + computeEffective(t) * t.multiplier,
    0,
  );
  const weightedRounded = Math.round(weightedEffective * 10) / 10;

  // Professions that have any modifier set
  const groupsWithModifiers = useMemo(() => {
    const set = new Set<string>();
    for (const row of trainerRows) {
      if (row.modified_ranks !== 0) set.add(row.profession);
    }
    return set;
  }, [trainerRows]);

  const allProfessions = useMemo(() => {
    const set = new Set<string>();
    for (const row of trainerRows) set.add(row.profession);
    return [...set];
  }, [trainerRows]);

  const openAll = useCallback(() => {
    setRankModifiersViewState({ collapsedGroups: [] });
  }, [setRankModifiersViewState]);

  const collapseAll = useCallback(() => {
    setRankModifiersViewState({ collapsedGroups: allProfessions });
  }, [allProfessions, setRankModifiersViewState]);

  const openAllSet = useCallback(() => {
    const collapsed = allProfessions.filter((p) => !groupsWithModifiers.has(p));
    setRankModifiersViewState({ collapsedGroups: collapsed });
  }, [allProfessions, groupsWithModifiers, setRankModifiersViewState]);

  const handleClearAllOverrides = useCallback(async () => {
    if (selectedCharacterId == null) return;
    const confirmed = await confirm(
      "This will reset all rank modifiers, override modes, and override dates back to defaults. Are you sure?",
      { title: "Clear All Overrides", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      await clearRankOverrides();
      const [updated, chars] = await Promise.all([
        getTrainers(selectedCharacterId),
        listCharacters(),
      ]);
      setTrainers(updated);
      setCharacters(chars);
      setRescanBanner(false);
    } catch (e) {
      console.error("Failed to clear rank overrides:", e);
    }
  }, [selectedCharacterId, setTrainers, setCharacters]);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-1 flex items-center justify-between">
        <div className="text-sm text-[var(--color-text-muted)]">
          {totalTrainers} trainers, {totalEffective.toLocaleString()} total ranks (
          {logRanks.toLocaleString()} from logs, {modifiedRanks.toLocaleString()}{" "}
          modified) | {weightedRounded} effective
        </div>
      </div>
      <div className="mb-3 text-xs text-[var(--color-text-muted)]">
        Add ranks that don't appear in logs. Use Override modes for characters with incomplete log history.
      </div>

      {rescanBanner && (
        <div className="mb-3 flex items-center justify-between rounded border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
          <span>Rescan logs to update rank counts for override trainers.</span>
          <button
            type="button"
            onClick={() => setRescanBanner(false)}
            className="ml-2 text-xs text-amber-400 hover:text-amber-200"
          >
            Dismiss
          </button>
        </div>
      )}

      <div className="mb-3 flex items-center gap-3">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search trainers..."
          className="w-full max-w-xs rounded border border-[var(--color-border)] bg-[var(--color-card)] px-3 py-1.5 text-sm transition-colors placeholder:text-[var(--color-text-muted)] focus:border-[var(--color-accent)] focus:outline-none"
        />
        {searchQuery.trim() && (
          <span className="text-xs text-[var(--color-text-muted)]">
            {filteredRows.length} matching
          </span>
        )}
        <div className="ml-auto flex items-center gap-1">
          <button type="button" onClick={openAll} className="rounded px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]">
            Open All
          </button>
          <button type="button" onClick={collapseAll} className="rounded px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]">
            Collapse All
          </button>
          <button type="button" onClick={openAllSet} className="rounded px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]" title="Open groups that have any modifiers set">
            Open All Set
          </button>
          <button
            type="button"
            onClick={handleClearAllOverrides}
            disabled={selectedCharacterId == null}
            className="ml-2 rounded border border-[var(--color-danger)] px-2 py-1 text-xs text-[var(--color-danger)] hover:bg-[var(--color-danger-bg)] disabled:opacity-40"
            title="Reset all rank modifiers, override modes, and override dates to defaults"
          >
            Clear All Overrides
          </button>
        </div>
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
                          <th className="px-3 py-2 text-left font-medium">
                            Mode / Value
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
                        {groupTrainers.map((t) => {
                          const isNonModifier = t.rank_mode !== "modifier";
                          const eff = computeEffective(t);
                          return (
                            <tr
                              key={t.name}
                              className={`border-b border-[var(--color-border)] last:border-b-0 ${
                                isNonModifier ? "border-l-2" : ""
                              } ${
                                t.rank_mode === "override"
                                  ? "border-l-amber-500"
                                  : t.rank_mode === "override_until_date"
                                    ? "border-l-blue-500"
                                    : ""
                              }`}
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
                              <td className={`px-3 py-1.5 text-right ${isNonModifier ? "opacity-40" : "text-[var(--color-text-muted)]"}`}>
                                {t.ranks}
                                {t.apply_learning_ranks > 0 && (
                                  <span className="text-[var(--color-text-muted)]"> +{t.apply_learning_ranks}a</span>
                                )}
                              </td>
                              <td className="px-3 py-1.5">
                                <RankModeInput
                                  row={t}
                                  onSave={(mode, val, date) => handleSave(t.name, mode, val, date)}
                                />
                              </td>
                              <td className="px-3 py-1.5 text-right font-medium">
                                {eff}
                              </td>
                              <td className="px-3 py-1.5 text-right text-[var(--color-text-muted)]">
                                {(() => {
                                  const weighted =
                                    Math.round(eff * t.multiplier * 10) / 10;
                                  return weighted % 1 === 0 ? weighted : weighted.toFixed(1);
                                })()}
                              </td>
                            </tr>
                          );
                        })}
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
