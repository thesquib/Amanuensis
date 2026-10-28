import { useMemo, useState, useEffect, useCallback } from "react";
import { confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../lib/store";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import {
  clearRankOverrides,
  discardMissingRankOverrides,
  exportRankOverrides,
  getMissingRankOverrides,
  getTrainerCheckpoints,
  getTrainerDbInfo,
  getTrainers,
  importRankOverrides,
  listCharacters,
  moveRankOverride,
  restoreMissingRankOverrides,
  setRankOverride,
} from "../../lib/commands";
import { PROFESSION_ORDER, STORAGE_KEYS } from "../../lib/constants";
import { todayMDYY } from "../../lib/dateUtils";
import type { RankOverride, Trainer, TrainerCheckpoint, TrainerInfo } from "../../types";

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
  /** Saved under a trainer name that's no longer in trainers.json (e.g. renamed). */
  orphan?: boolean;
}

const ORPHAN_GROUP = "Renamed or unknown";

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
  const { trainers, characters, setTrainers, setCharacters, selectedCharacterId, rankModifiersViewState, setRankModifiersViewState, showPlayerTrainers } = useStore();
  const { searchQuery, collapsedGroups: collapsedArr } = rankModifiersViewState;
  const collapsedGroups = useMemo(() => new Set(collapsedArr), [collapsedArr]);
  const setSearchQuery = useCallback((v: string) => setRankModifiersViewState({ searchQuery: v }), [setRankModifiersViewState]);
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);
  const [rescanBanner, setRescanBanner] = useState(false);
  const [missing, setMissing] = useState<RankOverride[]>([]);
  const [notice, setNotice] = useState<string | null>(null);
  // Bumped after a failed save to remount the inputs, so they show the stored value again.
  const [inputRev, setInputRev] = useState(0);
  const charName = characters.find((c) => c.id === selectedCharacterId)?.name ?? "this character";
  const [checkpointMap, setCheckpointMap] = useState<Map<string, TrainerCheckpoint>>(new Map());
  const hasSavedState = useMemo(() => localStorage.getItem(STORAGE_KEYS.COLLAPSED_RANK_MODIFIERS) !== null, []);
  const [defaultsInitialized, setDefaultsInitialized] = useState(hasSavedState);

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (selectedCharacterId == null) {
      setCheckpointMap(new Map());
      return;
    }
    getTrainerCheckpoints(selectedCharacterId, showPlayerTrainers)
      .then((checkpoints) => {
        const map = new Map<string, TrainerCheckpoint>();
        for (const cp of checkpoints) {
          map.set(cp.trainer_name, cp);
        }
        setCheckpointMap(map);
      })
      .catch(() => {});
  }, [selectedCharacterId, trainers, showPlayerTrainers]);

  // Backed-up overrides this character no longer has (cleared, or lost to a wipe or new database).
  useEffect(() => {
    if (selectedCharacterId == null) {
      setMissing([]);
      return;
    }
    getMissingRankOverrides(selectedCharacterId)
      .then(setMissing)
      .catch(() => setMissing([]));
  }, [selectedCharacterId, trainers]);

  const reload = useCallback(async () => {
    if (selectedCharacterId == null) return;
    const [updated, chars] = await Promise.all([
      getTrainers(selectedCharacterId),
      listCharacters(),
    ]);
    setTrainers(updated);
    setCharacters(chars);
  }, [selectedCharacterId, setTrainers, setCharacters]);

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
    // Overrides saved under a name trainers.json no longer has would otherwise be invisible.
    const known = new Set(trainerDb.map((t) => t.name));
    for (const t of trainers) {
      if (known.has(t.trainer_name) || (t.modified_ranks === 0 && t.rank_mode === "modifier")) continue;
      rows.push({
        name: t.trainer_name,
        profession: ORPHAN_GROUP,
        ranks: t.ranks,
        modified_ranks: t.modified_ranks,
        apply_learning_ranks: t.apply_learning_ranks,
        multiplier: t.effective_multiplier,
        is_combo: false,
        combo_components: [],
        rank_mode: t.rank_mode as RankMode,
        override_date: t.override_date,
        orphan: true,
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
        await reload();
        setNotice(null);
        if (mode !== "modifier") {
          setRescanBanner(true);
        }
      } catch (e) {
        setNotice(`Couldn't save ${trainerName}: ${e}. The value was not saved.`);
        setInputRev((r) => r + 1);
      }
    },
    [selectedCharacterId, reload],
  );

  const handleMove = useCallback(
    async (from: string, to: string) => {
      if (selectedCharacterId == null || !to) return;
      const ok = await confirm(`Move the rank override on "${from}" to "${to}"?`, { title: "Move Override" });
      if (!ok) return;
      try {
        await moveRankOverride(selectedCharacterId, from, to);
        await reload();
        setNotice(null);
      } catch (e) {
        setNotice(`Couldn't move ${from}: ${e}`);
      }
    },
    [selectedCharacterId, reload],
  );

  const handleRestoreMissing = useCallback(async () => {
    if (selectedCharacterId == null) return;
    try {
      const r = await restoreMissingRankOverrides(selectedCharacterId);
      await reload();
      if (r.needs_rescan) setRescanBanner(true);
    } catch (e) {
      setNotice(`Couldn't restore rank modifiers: ${e}`);
    }
  }, [selectedCharacterId, reload]);

  const handleDiscardMissing = useCallback(async () => {
    if (selectedCharacterId == null) return;
    const ok = await confirm(
      `Permanently forget the ${missing.length} saved rank modifier(s) for ${charName}? This cannot be undone.`,
      { title: "Discard Saved Modifiers", kind: "warning" },
    );
    if (!ok) return;
    try {
      await discardMissingRankOverrides(selectedCharacterId);
      setMissing([]);
    } catch (e) {
      setNotice(`Couldn't discard saved rank modifiers: ${e}`);
    }
  }, [selectedCharacterId, missing.length, charName]);

  const handleExport = useCallback(async () => {
    const path = await save({
      title: "Export Rank Modifiers",
      defaultPath: "rank-modifiers.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      const n = await exportRankOverrides(path);
      await message(`Exported ${n} rank modifier(s) for all characters to ${path}.`, { title: "Export Complete" });
    } catch (e) {
      await message(String(e), { title: "Export Failed", kind: "error" });
    }
  }, []);

  const handleImport = useCallback(async () => {
    const path = await open({
      title: "Import Rank Modifiers",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    const ok = await confirm(
      "Importing replaces the current value of every trainer in the file, for each character it names. Continue?",
      { title: "Import Rank Modifiers", kind: "warning" },
    );
    if (!ok) return;
    try {
      const r = await importRankOverrides(path);
      await reload();
      if (r.needs_rescan) setRescanBanner(true);
      const skipped = r.unknown_characters.length
        ? `\n\nSkipped characters not in this database: ${r.unknown_characters.join(", ")}`
        : "";
      await message(`Imported ${r.applied} rank modifier(s).${skipped}`, { title: "Import Complete" });
    } catch (e) {
      await message(String(e), { title: "Import Failed", kind: "error" });
    }
  }, [reload]);

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
      `Reset ${charName}'s rank modifiers, override modes, and override dates to defaults? Other characters are not affected, and you can restore them afterwards.`,
      { title: "Clear Overrides", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      await clearRankOverrides(selectedCharacterId);
      await reload();
      setRescanBanner(false);
    } catch (e) {
      setNotice(`Couldn't clear rank overrides: ${e}`);
    }
  }, [selectedCharacterId, charName, reload]);

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

      {notice && (
        <div className="mb-3 flex items-center justify-between rounded border border-[var(--color-danger)] bg-[var(--color-danger-bg)] px-3 py-2 text-sm text-[var(--color-danger)]">
          <span>{notice}</span>
          <button type="button" onClick={() => setNotice(null)} className="ml-2 text-xs hover:opacity-70">
            Dismiss
          </button>
        </div>
      )}

      {missing.length > 0 && (
        <div className="mb-3 flex items-center justify-between rounded border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-300">
          <span title={missing.map((m) => `${m.trainer}: ${m.modified_ranks} (${m.rank_mode})`).join("\n")}>
            {missing.length} saved rank modifier(s) for {charName} aren't in this database.
          </span>
          <div className="ml-2 flex shrink-0 gap-3 text-xs">
            <button type="button" onClick={handleRestoreMissing} className="text-amber-200 hover:text-white">
              Restore
            </button>
            <button type="button" onClick={handleDiscardMissing} className="text-amber-400 hover:text-amber-200">
              Discard
            </button>
          </div>
        </div>
      )}

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
          <button type="button" onClick={handleExport} className="ml-2 rounded px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]" title="Save every character's rank modifiers to a file">
            Export
          </button>
          <button type="button" onClick={handleImport} className="rounded px-2 py-1 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]" title="Load rank modifiers from an exported file">
            Import
          </button>
          <button
            type="button"
            onClick={handleClearAllOverrides}
            disabled={selectedCharacterId == null}
            className="ml-2 rounded border border-[var(--color-danger)] px-2 py-1 text-xs text-[var(--color-danger)] hover:bg-[var(--color-danger-bg)] disabled:opacity-40"
            title="Reset this character's rank modifiers, override modes, and override dates to defaults"
          >
            Clear Overrides
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
                          <th className="w-28 px-3 py-2 text-right font-medium">
                            Checkpoint
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
                                {t.orphan && (
                                  <select
                                    value=""
                                    onChange={(e) => handleMove(t.name, e.target.value)}
                                    className="ml-2 rounded border border-[var(--color-border)] bg-[var(--color-card)] px-1 py-0.5 text-xs"
                                    title="This trainer name isn't in the trainer list any more. Move the override to the current name."
                                  >
                                    <option value="">Move to…</option>
                                    {trainerDb.map((d) => (
                                      <option key={d.name} value={d.name}>{d.name}</option>
                                    ))}
                                  </select>
                                )}
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
                                  key={inputRev}
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
                              <td className="px-3 py-1.5 text-right">
                                {(() => {
                                  const cp = checkpointMap.get(t.name);
                                  if (!cp) {
                                    return <span className="text-[var(--color-text-muted)]">—</span>;
                                  }
                                  const inRange = eff >= cp.rank_min && (cp.rank_max === null || eff <= cp.rank_max);
                                  const rangeText = cp.rank_max === null ? "Maxed" : `${cp.rank_min}–${cp.rank_max}`;
                                  return (
                                    <span
                                      className={inRange ? "text-green-400" : "text-amber-400"}
                                      title={(() => {
                                        const d = cp.timestamp.slice(0, 10).split("-");
                                        const months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
                                        const label = `${months[parseInt(d[1]) - 1]} ${parseInt(d[2])}, ${d[0]}`;
                                        return `Checkpoint from ${label} — ${rangeText}`;
                                      })()}
                                    >
                                      {rangeText}
                                    </span>
                                  );
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
