import { useMemo, useEffect, useState } from "react";
import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
} from "recharts";
import { useStore } from "../../lib/store";
import { getTrainerDbInfo } from "../../lib/commands";
import { SP_COSTS, RACE_SP, computeFighterStats } from "../../lib/fighterStats";
import type { TrainerInfo, Kill, Trainer, Lasty } from "../../types";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CVPoint {
  date: string;
  killCv: number | null;
  rankCv: number | null;
  killCreature?: string;
}

type TrainerPoint = Record<string, number | string | null>;

interface StudiesPoint {
  date: string;
  movements: number;
  befriends: number;
  morphs: number;
  movementCreature?: string;
  befriendCreature?: string;
  morphCreature?: string;
}

interface StatsPoint {
  date: string;
  trainedRanks: number;
  effectiveRanks: number;
  slaughterRanks: number;
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

const TRAINER_COLORS = [
  "#60a5fa", "#34d399", "#f87171", "#fbbf24", "#fb923c",
  "#38bdf8", "#4ade80", "#f472b6", "#c084fc", "#94a3b8",
];

// ---------------------------------------------------------------------------
// Data builders
// ---------------------------------------------------------------------------

function buildCVTimeline(
  kills: Kill[],
  trainers: Trainer[],
): CVPoint[] {
  // Kill CV: running max of creature_value (killed + assisted_kill), sorted by date_first
  const killSteps: { date: string; cv: number; creature: string }[] = [];
  const sortedKills = kills
    .filter((k) => k.date_first != null && (k.killed_count + k.assisted_kill_count) > 0)
    .sort((a, b) => a.date_first!.localeCompare(b.date_first!));

  let runningMax = 0;
  for (const k of sortedKills) {
    if (k.creature_value > runningMax) {
      runningMax = k.creature_value;
      killSteps.push({ date: k.date_first!.slice(0, 10), cv: k.creature_value, creature: k.creature_name });
    }
  }

  // Rank CV: Est. Slaughter Points / 150 — same formula as Summary "Ranks"
  // RACE_SP is a constant baseline present from the start
  const rankCvBase = RACE_SP / 150;

  const spTarget =
    (RACE_SP +
      trainers.reduce((sum, t) => {
        const sp = SP_COSTS[t.trainer_name] ?? 0;
        return sum + (t.ranks + t.modified_ranks) * sp;
      }, 0)) /
    150;

  const trainerSteps: { date: string; cv: number }[] = [];
  const datedTrainers = trainers
    .filter((t) => t.date_of_last_rank != null && (t.ranks + t.modified_ranks) > 0 && (SP_COSTS[t.trainer_name] ?? 0) > 0)
    .sort((a, b) => a.date_of_last_rank!.localeCompare(b.date_of_last_rank!));

  let cumulative = rankCvBase;
  for (const t of datedTrainers) {
    cumulative += (t.ranks + t.modified_ranks) * (SP_COSTS[t.trainer_name] ?? 0) / 150;
    trainerSteps.push({ date: t.date_of_last_rank!.slice(0, 10), cv: cumulative });
  }

  if (killSteps.length === 0 && trainerSteps.length === 0) return [];

  // Prepend a baseline rank CV point at the earliest known date
  const allKnownDates = [...killSteps.map((p) => p.date), ...trainerSteps.map((p) => p.date)].sort();
  const earliestDate = allKnownDates[0];
  const latestDate = allKnownDates[allKnownDates.length - 1];

  if (trainerSteps.length === 0 || trainerSteps[0].date > earliestDate) {
    trainerSteps.unshift({ date: earliestDate, cv: rankCvBase });
  }

  // Undated SP trainers: add remainder at latest date
  const remainder = spTarget - cumulative;
  if (remainder > 0.01 && latestDate) {
    trainerSteps.push({ date: latestDate, cv: spTarget });
  }

  // Merge into unified forward-filled timeline
  const allDates = [...new Set([
    ...killSteps.map((p) => p.date),
    ...trainerSteps.map((p) => p.date),
  ])].sort();

  let lastKill: number | null = null;
  let lastRank: number | null = null;
  let lastKillCreature: string | undefined;

  return allDates.map((date) => {
    const kMatches = killSteps.filter((p) => p.date === date);
    const rMatches = trainerSteps.filter((p) => p.date === date);

    let killCreatureChanged: string | undefined;
    if (kMatches.length > 0) {
      const last = kMatches[kMatches.length - 1];
      lastKill = last.cv;
      lastKillCreature = last.creature;
      killCreatureChanged = last.creature;
    }
    if (rMatches.length > 0) {
      lastRank = rMatches[rMatches.length - 1].cv;
    }
    void lastKillCreature;
    return {
      date,
      killCv: lastKill,
      rankCv: lastRank != null ? Math.round(lastRank * 10) / 10 : null,
      ...(killCreatureChanged != null && { killCreature: killCreatureChanged }),
    };
  });
}

function buildTrainerTimeline(trainers: Trainer[], topNames: string[]): TrainerPoint[] {
  const datedTrainers = trainers
    .filter((t) => t.date_of_last_rank != null && (t.ranks + t.modified_ranks) > 0 && topNames.includes(t.trainer_name))
    .sort((a, b) => a.date_of_last_rank!.localeCompare(b.date_of_last_rank!));

  if (datedTrainers.length === 0) return [];

  const allDates = [...new Set(datedTrainers.map((t) => t.date_of_last_rank!.slice(0, 10)))].sort();

  // Prepend an origin point one day before the earliest trainer date, all trainers at 0.
  // This ensures lines step up from 0 rather than appearing flat from the start.
  const firstD = new Date(allDates[0] + "T12:00:00");
  firstD.setDate(firstD.getDate() - 1);
  const originDate = firstD.toISOString().slice(0, 10);

  const originPoint: TrainerPoint = { date: originDate };
  for (const name of topNames) originPoint[name] = 0;

  const current: Record<string, number> = {};
  const result: TrainerPoint[] = [originPoint];

  for (const date of allDates) {
    for (const t of datedTrainers) {
      if (t.date_of_last_rank!.slice(0, 10) === date) {
        current[t.trainer_name] = (current[t.trainer_name] ?? 0) + (t.ranks + t.modified_ranks);
      }
    }
    const point: TrainerPoint = { date };
    for (const name of topNames) {
      // Use 0 for trainers not yet reached their date (not null) so lines stay at 0
      point[name] = current[name] ?? 0;
    }
    result.push(point);
  }

  return result;
}

function buildStudiesTimeline(lastys: Lasty[]): StudiesPoint[] {
  const byType = (type: string) =>
    lastys
      .filter((l) => l.lasty_type === type && l.finished && l.completed_date)
      .sort((a, b) => a.completed_date!.localeCompare(b.completed_date!));

  const movs = byType("Movements").map((l, i) => ({ date: l.completed_date!.slice(0, 10), count: i + 1, creature: l.creature_name }));
  const befs = byType("Befriend").map((l, i) => ({ date: l.completed_date!.slice(0, 10), count: i + 1, creature: l.creature_name }));
  const morphs = byType("Morph").map((l, i) => ({ date: l.completed_date!.slice(0, 10), count: i + 1, creature: l.creature_name }));

  const allDates = [...new Set([...movs, ...befs, ...morphs].map((s) => s.date))].sort();
  if (allDates.length === 0) return [];

  let lm = 0, lb = 0, lmo = 0;

  return allDates.map((date) => {
    const m = movs.filter((s) => s.date === date);
    const b = befs.filter((s) => s.date === date);
    const mo = morphs.filter((s) => s.date === date);
    if (m.length) lm = m[m.length - 1].count;
    if (b.length) lb = b[b.length - 1].count;
    if (mo.length) lmo = mo[mo.length - 1].count;
    return {
      date,
      movements: lm,
      befriends: lb,
      morphs: lmo,
      ...(m.length && { movementCreature: m[m.length - 1].creature }),
      ...(b.length && { befriendCreature: b[b.length - 1].creature }),
      ...(mo.length && { morphCreature: mo[mo.length - 1].creature }),
    };
  });
}

function buildStatsTimeline(trainers: Trainer[], trainerDb: TrainerInfo[]): StatsPoint[] {
  const multMap = new Map(trainerDb.map((t) => [t.name, t.multiplier]));
  const datedTrainers = trainers
    .filter((t) => t.date_of_last_rank != null && (t.ranks + t.modified_ranks) > 0)
    .sort((a, b) => a.date_of_last_rank!.localeCompare(b.date_of_last_rank!));

  const allDates = [...new Set(datedTrainers.map((t) => t.date_of_last_rank!.slice(0, 10)))].sort();
  if (allDates.length === 0) return [];

  return allDates.map((date) => {
    const ranksAtDate = new Map<string, number>();
    for (const t of datedTrainers) {
      if (t.date_of_last_rank!.slice(0, 10) <= date) {
        ranksAtDate.set(t.trainer_name, (ranksAtDate.get(t.trainer_name) ?? 0) + (t.ranks + t.modified_ranks));
      }
    }
    const stats = computeFighterStats(ranksAtDate, multMap);
    return {
      date,
      trainedRanks: stats.trainedRanks,
      effectiveRanks: Math.round(stats.effectiveRanks),
      slaughterRanks: Math.round(stats.slaughterPoints / 150),
    };
  });
}

// ---------------------------------------------------------------------------
// Formatters & helpers
// ---------------------------------------------------------------------------

function formatDateTick(dateStr: string): string {
  const [year, month] = dateStr.split("-");
  const months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  return `${months[parseInt(month) - 1]} ${year}`;
}

function pickXTicks(dates: string[], count = 8): string[] {
  if (dates.length === 0) return [];
  if (dates.length <= count) return dates;
  const step = Math.floor(dates.length / (count - 1));
  const ticks: string[] = [];
  for (let i = 0; i < dates.length; i += step) ticks.push(dates[i]);
  if (ticks[ticks.length - 1] !== dates[dates.length - 1]) ticks.push(dates[dates.length - 1]);
  return ticks;
}

// ---------------------------------------------------------------------------
// Custom tick
// ---------------------------------------------------------------------------

interface CustomYTickProps { x?: number; y?: number; payload?: { value: number }; step?: number }

function CustomYTick({ x = 0, y = 0, payload, step = 100 }: CustomYTickProps) {
  if (!payload) return null;
  const isMajor = payload.value % step === 0;
  return (
    <text x={x} y={y} dy={4} textAnchor="end" fill="var(--color-text-muted)"
      fontSize={isMajor ? 12 : 9} fontWeight={isMajor ? 600 : 400}>
      {isMajor ? payload.value : "·"}
    </text>
  );
}

function yTicks(maxVal: number, minor: number): number[] {
  const top = Math.ceil(maxVal / minor) * minor;
  return Array.from({ length: top / minor + 1 }, (_, i) => i * minor);
}

// ---------------------------------------------------------------------------
// Tooltips
// ---------------------------------------------------------------------------

function CVTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; payload: CVPoint }>; label?: string }) {
  if (!active || !payload?.length || !label) return null;
  const point = payload[0]?.payload as CVPoint | undefined;
  const kill = payload.find((p) => p.name === "Kill CV");
  const rank = payload.find((p) => p.name === "Rank CV");
  return (
    <div className="rounded border border-[var(--color-border)] bg-[var(--color-card)] p-2 text-sm shadow-md">
      <div className="mb-1 font-medium text-[var(--color-text)]">{formatDateTick(label)}</div>
      {kill?.value != null && (
        <div className="text-[var(--color-accent)]">
          Kill CV: <span className="font-semibold">{kill.value}</span>
          {point?.killCreature && <span className="ml-1 text-[var(--color-text-muted)]">— {point.killCreature}</span>}
        </div>
      )}
      {rank?.value != null && (
        <div style={{ color: "#a78bfa" }}>
          Rank CV: <span className="font-semibold">{rank.value}</span>
        </div>
      )}
    </div>
  );
}

function TrainerTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string }>; label?: string }) {
  if (!active || !payload?.length || !label) return null;
  const entries = payload.filter((p) => p.value != null && p.value > 0);
  if (entries.length === 0) return null;
  return (
    <div className="rounded border border-[var(--color-border)] bg-[var(--color-card)] p-2 text-sm shadow-md">
      <div className="mb-1 font-medium text-[var(--color-text)]">{formatDateTick(label)}</div>
      {entries.map((e) => (
        <div key={e.name} style={{ color: e.color }}>
          {e.name}: <span className="font-semibold">{e.value}</span>
        </div>
      ))}
    </div>
  );
}

function StudiesTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string; payload: StudiesPoint }>; label?: string }) {
  if (!active || !payload?.length || !label) return null;
  const point = payload[0]?.payload as StudiesPoint | undefined;
  return (
    <div className="rounded border border-[var(--color-border)] bg-[var(--color-card)] p-2 text-sm shadow-md">
      <div className="mb-1 font-medium text-[var(--color-text)]">{formatDateTick(label)}</div>
      {payload.filter((p) => p.value != null).map((e) => {
        let detail: string | undefined;
        if (e.name === "Movements") detail = point?.movementCreature;
        if (e.name === "Befriends") detail = point?.befriendCreature;
        if (e.name === "Morphs") detail = point?.morphCreature;
        return (
          <div key={e.name} style={{ color: e.color }}>
            {e.name}: <span className="font-semibold">{e.value}</span>
            {detail && <span className="ml-1 text-[var(--color-text-muted)]">— {detail}</span>}
          </div>
        );
      })}
    </div>
  );
}

function StatsTooltip({ active, payload, label }: { active?: boolean; payload?: Array<{ value: number; name: string; color: string }>; label?: string }) {
  if (!active || !payload?.length || !label) return null;
  return (
    <div className="rounded border border-[var(--color-border)] bg-[var(--color-card)] p-2 text-sm shadow-md">
      <div className="mb-1 font-medium text-[var(--color-text)]">{formatDateTick(label)}</div>
      {payload.map((e) => (
        <div key={e.name} style={{ color: e.color }}>
          {e.name}: <span className="font-semibold">{e.value?.toLocaleString()}</span>
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Checkbox toggle
// ---------------------------------------------------------------------------

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex cursor-pointer items-center gap-2 text-sm text-[var(--color-text-muted)]">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)}
        className="h-4 w-4 accent-[var(--color-accent)]" />
      {label}
    </label>
  );
}

// ---------------------------------------------------------------------------
// Chart section wrapper
// ---------------------------------------------------------------------------

function ChartSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-4">
      <div className="mb-3 text-sm font-semibold text-[var(--color-text-muted)] uppercase tracking-wide">{title}</div>
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

export function CVGraphView() {
  const { kills, trainers, lastys, characters, selectedCharacterId } = useStore();
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);
  const [showTrainers, setShowTrainers] = useState(false);
  const [showStats, setShowStats] = useState(false);

  useEffect(() => {
    getTrainerDbInfo().then(setTrainerDb).catch(() => {});
  }, []);

  const profession = characters.find((c) => c.id === selectedCharacterId)?.profession ?? "";
  const isRanger = profession === "Ranger";

  // Top trainer names (by rank count, with a date, capped at 10)
  const topTrainerNames = useMemo(() =>
    [...trainers]
      .filter((t) => t.date_of_last_rank != null && (t.ranks + t.modified_ranks) > 0)
      .sort((a, b) => (b.ranks + b.modified_ranks) - (a.ranks + a.modified_ranks))
      .slice(0, 10)
      .map((t) => t.trainer_name),
    [trainers],
  );

  const cvData = useMemo(() => buildCVTimeline(kills, trainers), [kills, trainers]);
  const trainerData = useMemo(() => showTrainers ? buildTrainerTimeline(trainers, topTrainerNames) : [], [trainers, topTrainerNames, showTrainers]);
  const studiesData = useMemo(() => (showTrainers && isRanger) ? buildStudiesTimeline(lastys) : [], [lastys, showTrainers, isRanger]);
  const statsData = useMemo(() => showStats ? buildStatsTimeline(trainers, trainerDb) : [], [trainers, trainerDb, showStats]);

  const cvMaxCv = useMemo(() => Math.max(0, ...cvData.map((p) => Math.max(p.killCv ?? 0, p.rankCv ?? 0))), [cvData]);
  const cvYTicks = useMemo(() => yTicks(cvMaxCv, 20), [cvMaxCv]);
  const cvXTicks = useMemo(() => pickXTicks(cvData.map((d) => d.date)), [cvData]);

  const trainerMax = useMemo(() => {
    if (!trainerData.length) return 100;
    return Math.max(100, ...trainerData.flatMap((p) =>
      topTrainerNames.map((n) => (p[n] as number | null) ?? 0)
    ));
  }, [trainerData, topTrainerNames]);
  const trainerYTicks = useMemo(() => yTicks(trainerMax, 50), [trainerMax]);
  const trainerXTicks = useMemo(() => pickXTicks(trainerData.map((d) => d.date as string)), [trainerData]);

  const studiesMax = useMemo(() => Math.max(5, ...studiesData.map((p) => Math.max(p.movements, p.befriends, p.morphs))), [studiesData]);
  const studiesXTicks = useMemo(() => pickXTicks(studiesData.map((d) => d.date)), [studiesData]);

  const statsMax = useMemo(() => statsData.length ? Math.max(100, ...statsData.map((p) => Math.max(p.trainedRanks, p.effectiveRanks))) : 100, [statsData]);
  const statsXTicks = useMemo(() => pickXTicks(statsData.map((d) => d.date)), [statsData]);

  if (cvData.length === 0) {
    return (
      <div className="flex h-64 items-center justify-center text-[var(--color-text-muted)]">
        No dated kill or trainer data available for this character.
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Header + toggles */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-[var(--color-text)]">CV Over Time</h2>
        <div className="flex gap-4">
          <Toggle label="Trainers" checked={showTrainers} onChange={setShowTrainers} />
          <Toggle label="Stats" checked={showStats} onChange={setShowStats} />
        </div>
      </div>

      {/* Main CV chart */}
      <ChartSection title="Coin Value">
        <ResponsiveContainer width="100%" height={380}>
          <LineChart data={cvData} margin={{ top: 10, right: 20, left: 10, bottom: 0 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
            <XAxis dataKey="date" ticks={cvXTicks} tickFormatter={formatDateTick}
              tick={{ fill: "var(--color-text-muted)", fontSize: 11 }} stroke="var(--color-border)" />
            <YAxis ticks={cvYTicks} tick={<CustomYTick step={100} />} stroke="var(--color-border)" width={42} />
            <Tooltip content={<CVTooltip />} />
            <Legend wrapperStyle={{ color: "var(--color-text-muted)", fontSize: 12 }} />
            <Line type="stepAfter" dataKey="killCv" name="Kill CV" stroke="var(--color-accent)"
              dot={false} activeDot={{ r: 5 }} connectNulls />
            <Line type="stepAfter" dataKey="rankCv" name="Rank CV" stroke="#a78bfa"
              strokeDasharray="5 5" dot={false} activeDot={{ r: 5 }} connectNulls />
          </LineChart>
        </ResponsiveContainer>
      </ChartSection>

      {/* Trainer ranks chart */}
      {showTrainers && trainerData.length > 0 && (
        <ChartSection title="Individual Trainer Ranks">
          <ResponsiveContainer width="100%" height={300}>
            <LineChart data={trainerData} margin={{ top: 10, right: 20, left: 10, bottom: 0 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
              <XAxis dataKey="date" ticks={trainerXTicks} tickFormatter={formatDateTick}
                tick={{ fill: "var(--color-text-muted)", fontSize: 11 }} stroke="var(--color-border)" />
              <YAxis ticks={trainerYTicks} tick={<CustomYTick step={100} />} stroke="var(--color-border)" width={42} />
              <Tooltip content={<TrainerTooltip />} />
              <Legend wrapperStyle={{ color: "var(--color-text-muted)", fontSize: 11 }} />
              {topTrainerNames.map((name, i) => (
                <Line key={name} type="stepAfter" dataKey={name} name={name}
                  stroke={TRAINER_COLORS[i % TRAINER_COLORS.length]}
                  dot={false} activeDot={{ r: 4 }} />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartSection>
      )}

      {/* Ranger studies chart */}
      {showTrainers && isRanger && studiesData.length > 0 && (
        <ChartSection title="Ranger Studies">
          <ResponsiveContainer width="100%" height={220}>
            <LineChart data={studiesData} margin={{ top: 10, right: 20, left: 10, bottom: 0 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
              <XAxis dataKey="date" ticks={studiesXTicks} tickFormatter={formatDateTick}
                tick={{ fill: "var(--color-text-muted)", fontSize: 11 }} stroke="var(--color-border)" />
              <YAxis allowDecimals={false} domain={[0, studiesMax + 1]} stroke="var(--color-border)" width={30}
                tick={{ fill: "var(--color-text-muted)", fontSize: 11 }} />
              <Tooltip content={<StudiesTooltip />} />
              <Legend wrapperStyle={{ color: "var(--color-text-muted)", fontSize: 12 }} />
              <Line type="stepAfter" dataKey="movements" name="Movements" stroke="#34d399" dot={false} activeDot={{ r: 4 }} connectNulls />
              <Line type="stepAfter" dataKey="befriends" name="Befriends" stroke="#60a5fa" dot={false} activeDot={{ r: 4 }} connectNulls />
              <Line type="stepAfter" dataKey="morphs" name="Morphs" stroke="#f472b6" dot={false} activeDot={{ r: 4 }} connectNulls />
            </LineChart>
          </ResponsiveContainer>
        </ChartSection>
      )}

      {/* Stats over time */}
      {showStats && statsData.length > 0 && (
        <ChartSection title="Rank Progression">
          <ResponsiveContainer width="100%" height={280}>
            <LineChart data={statsData} margin={{ top: 10, right: 20, left: 10, bottom: 0 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="var(--color-border)" />
              <XAxis dataKey="date" ticks={statsXTicks} tickFormatter={formatDateTick}
                tick={{ fill: "var(--color-text-muted)", fontSize: 11 }} stroke="var(--color-border)" />
              <YAxis ticks={yTicks(statsMax, statsMax > 2000 ? 500 : 100)}
                stroke="var(--color-border)" width={50}
                tick={{ fill: "var(--color-text-muted)", fontSize: 11 }} />
              <Tooltip content={<StatsTooltip />} />
              <Legend wrapperStyle={{ color: "var(--color-text-muted)", fontSize: 12 }} />
              <Line type="stepAfter" dataKey="trainedRanks" name="Trained Ranks" stroke="#fbbf24" dot={false} activeDot={{ r: 4 }} connectNulls />
              <Line type="stepAfter" dataKey="effectiveRanks" name="Effective Ranks" stroke="#34d399" dot={false} activeDot={{ r: 4 }} connectNulls />
              <Line type="stepAfter" dataKey="slaughterRanks" name="Est. Slaughter Ranks" stroke="#a78bfa"
                strokeDasharray="5 5" dot={false} activeDot={{ r: 4 }} connectNulls />
            </LineChart>
          </ResponsiveContainer>
        </ChartSection>
      )}

      <p className="text-xs text-[var(--color-text-muted)]">
        Rank CV = Est. Slaughter Points / 150. All trainer values are approximate — full ranks appear at date of last recorded rank.
      </p>
    </div>
  );
}
