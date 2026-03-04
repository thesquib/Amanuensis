import { useState, useEffect, useMemo, useCallback } from "react";
import { useStore } from "../../lib/store";
import { StatCard } from "../shared/StatCard";
import { KillTypePanel } from "../shared/KillTypePanel";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import { CreatureImage } from "../shared/CreatureImage";
import { CharacterPortrait } from "../shared/CharacterPortrait";
import {
  getTrainerDbInfo,
  getMergeSources,
  unmergeCharacter,
  listCharacters,
  getCharacterMerged,
  getKills,
  getTrainers,
  getPets,
  getLastys,
} from "../../lib/commands";
import { computeKillStats } from "../../lib/killStats";
import { computeFighterStats } from "../../lib/fighterStats";
import { timeAgo } from "../../lib/timeAgo";
import type { Character, TrainerInfo } from "../../types";

export function SummaryView() {
  const {
    characters,
    selectedCharacterId,
    kills,
    trainers,
    setCharacters,
    setKills,
    setTrainers,
    setPets,
    setLastys,
  } = useStore();
  const [trainerDb, setTrainerDb] = useState<TrainerInfo[]>([]);
  const [mergeSources, setMergeSources] = useState<Character[]>([]);
  const [mergedChar, setMergedChar] = useState<Character | null>(null);

  useEffect(() => {
    getTrainerDbInfo()
      .then(setTrainerDb)
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (selectedCharacterId !== null) {
      getMergeSources(selectedCharacterId)
        .then(setMergeSources)
        .catch(() => setMergeSources([]));
      getCharacterMerged(selectedCharacterId)
        .then(setMergedChar)
        .catch(() => setMergedChar(null));
    } else {
      setMergeSources([]);
      setMergedChar(null);
    }
  }, [selectedCharacterId]);

  const handleUnmerge = useCallback(
    async (sourceId: number) => {
      try {
        await unmergeCharacter(sourceId);
        // Refresh everything
        const chars = await listCharacters();
        setCharacters(chars);
        if (selectedCharacterId !== null) {
          // Reload merge sources and merged character stats
          const [sources, mc, k, t, p, l] = await Promise.all([
            getMergeSources(selectedCharacterId),
            getCharacterMerged(selectedCharacterId),
            getKills(selectedCharacterId),
            getTrainers(selectedCharacterId),
            getPets(selectedCharacterId),
            getLastys(selectedCharacterId),
          ]);
          setMergeSources(sources);
          setMergedChar(mc);
          setKills(k);
          setTrainers(t);
          setPets(p);
          setLastys(l);
        }
      } catch (e) {
        console.error("Unmerge failed:", e);
      }
    },
    [selectedCharacterId, setCharacters, setKills, setTrainers, setPets, setLastys],
  );

  const baseChar = characters.find((c) => c.id === selectedCharacterId);
  if (!baseChar) return null;
  // Use merged stats when available (aggregated logins, deaths, etc.)
  const char = mergedChar ?? baseChar;

  const {
    totalSolo: totalKills,
    totalAssisted,
    uniqueCreatures,
    nemesis,
    highestKilled,
    coinLevelKill,
    highestSlaughtered,
    highestVanquished,
    highestDispatched,
    lowestRecentKill,
    lowestRecentSlaughtered,
    lowestRecentVanquished,
    lowestRecentDispatched,
    mostKilled,
    mostKilledTotal,
    highestSoloKill,
    mostSoloKilled,
    mostSoloKilledTotal,
    mostRecentSoloKill,
    mostRecentAssistedKill,
    highestLootKill,
  } = useMemo(() => computeKillStats(kills), [kills]);

  // Prefer the TS-computed coinLevelKill.creature_value over char.coin_level:
  // the Rust SQL has no stuffable filter (no family data in creatures.csv), so it can
  // include non-stuffable creatures (e.g. Ghastly Presence at 650). The TS computation
  // correctly excludes those and is the authoritative displayed value.
  const confirmedCoinLevel = coinLevelKill?.creature_value ?? char.coin_level;
  const displayCoinLevel = confirmedCoinLevel > 0 ? confirmedCoinLevel : char.coin_level_interim;
  const coinLevelEstimated = confirmedCoinLevel === 0 && char.coin_level_interim > 0;

  const totalRanks = trainers.reduce(
    (sum, t) => sum + t.ranks + t.modified_ranks,
    0,
  );

  const { effectiveRanks, slaughterPoints } = useMemo(() => {
    const ranksMap = new Map<string, number>();
    const multMap = new Map<string, number>();
    for (const t of trainerDb) multMap.set(t.name, t.multiplier);
    for (const t of trainers) {
      const total = t.ranks + t.modified_ranks;
      ranksMap.set(t.trainer_name, (ranksMap.get(t.trainer_name) ?? 0) + total);
    }
    const stats = computeFighterStats(ranksMap, multMap);
    const effectiveRanks = trainers.reduce(
      (sum, t) => sum + (t.ranks + t.modified_ranks) * (multMap.get(t.trainer_name) ?? 1.0),
      0,
    );
    return { effectiveRanks, slaughterPoints: stats.slaughterPoints };
  }, [trainers, trainerDb]);

  const effectiveRounded = Math.round(effectiveRanks * 10) / 10;

  // Computed percentages
  const chanceOfDepart =
    char.deaths + char.departs > 0
      ? ((char.departs / (char.deaths + char.departs)) * 100).toFixed(1)
      : null;

  const chanceOfChainBreak =
    char.chains_used + char.chains_broken > 0
      ? (
          (char.chains_broken / (char.chains_used + char.chains_broken)) *
          100
        ).toFixed(1)
      : null;

  return (
    <div>
      <div className="mb-4 flex items-center gap-4">
        <div>
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-bold">{char.name}</h2>
            <ProfessionBadge profession={char.profession} />
          </div>
          {char.start_date && (
            <p className="text-[var(--color-text-muted)] mt-1 text-sm">
              Playing since {char.start_date.split(" ")[0]}
            </p>
          )}
        </div>
      </div>

      {mergeSources.length > 0 && (
        <div className="mb-4 rounded border border-[var(--color-border)] bg-[var(--color-card)]/30 px-3 py-2">
          <div className="mb-1 text-xs font-medium text-[var(--color-text-muted)]">
            Merged from:
          </div>
          <div className="flex flex-wrap gap-2">
            {mergeSources.map((source) => (
              <span
                key={source.id}
                className="inline-flex items-center gap-1.5 rounded bg-[var(--color-card)] px-2 py-1 text-xs"
              >
                {source.name}
                <button
                  onClick={() => source.id !== null && handleUnmerge(source.id)}
                  className="rounded px-1 text-[var(--color-text-muted)] hover:bg-[var(--color-danger-bg)] hover:text-[var(--color-danger)]"
                  title={`Unmerge ${source.name}`}
                >
                  &times;
                </button>
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">

        {/* ── Portrait + top stats ─────────────────────────────────── */}
        <div className="row-span-2 flex items-center justify-center rounded-lg bg-[var(--color-card)] p-4">
          <CharacterPortrait
            name={char.name}
            className="h-full max-h-40 w-auto rounded-lg"
          />
        </div>
        <div className="rounded-lg bg-[var(--color-card)] p-4 flex flex-col justify-between">
          <div>
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)]">Coin Level</div>
            <div className="mt-1 text-4xl font-bold">
              {displayCoinLevel.toLocaleString()}{coinLevelEstimated && <span className="text-[var(--color-accent)] text-2xl">*</span>}
            </div>
            <div className="mt-0.5 text-xs text-[var(--color-text-muted)]">
              {coinLevelEstimated ? <span className="text-[var(--color-accent)]">*not enough data yet</span> : "Highest Kill"}
            </div>
            {(() => {
              // Confirmed level → creature that actually set it (≥5 verb kills).
              // Interim level   → best available verb-kill creature (≥1 kill).
              const clCreature = coinLevelEstimated ? highestKilled : coinLevelKill;
              return clCreature ? (
                <div className="mt-2 flex items-center gap-1.5">
                  <CreatureImage creatureName={clCreature.creature_name} className="h-6 w-auto" />
                  <span className="text-xs text-[var(--color-text-muted)] truncate">{clCreature.creature_name}</span>
                </div>
              ) : null;
            })()}
          </div>
          <div className="mt-3 border-t border-[var(--color-border)] pt-3">
            <div className="mt-1 text-4xl font-bold">{Math.round(slaughterPoints / 150).toLocaleString()}</div>
            <div className="mt-0.5 text-xs text-[var(--color-text-muted)]">Ranks</div>
          </div>
        </div>
        <div className="rounded-lg bg-[var(--color-card)] px-3 py-2 flex flex-col justify-center gap-1">
          <div className="flex items-center justify-between gap-2">
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">Logins</div>
            <div className="text-sm font-semibold">{char.logins.toLocaleString()}</div>
          </div>
          <div className="flex items-center justify-between gap-2">
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">Deaths</div>
            <div className="text-sm font-semibold">{char.deaths.toLocaleString()}</div>
          </div>
          <div className="flex items-center justify-between gap-2">
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">Departs</div>
            <div className="text-sm font-semibold">{char.departs.toLocaleString()}</div>
          </div>
          <div className="my-0.5 border-t border-[var(--color-border)]" />
          <div className="flex items-center justify-between gap-2">
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">Good Karma</div>
            <div className="text-sm font-semibold">{char.good_karma.toLocaleString()}</div>
          </div>
          <div className="flex items-center justify-between gap-2">
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">Bad Karma</div>
            <div className="text-sm font-semibold">{char.bad_karma.toLocaleString()}</div>
          </div>
          <div className="flex items-center justify-between gap-2">
            <div className="text-xs uppercase tracking-wide text-[var(--color-text-muted)] shrink-0">Esteem</div>
            <div className="text-sm font-semibold">{char.esteem.toLocaleString()}</div>
          </div>
        </div>
        <StatCard
          label="Solo Kills"
          value={totalKills.toLocaleString()}
          sub={[
            `${uniqueCreatures} unique creatures`,
            mostRecentSoloKill?.creature_name,
            timeAgo(mostRecentSoloKill?.date_last),
          ].filter(Boolean).join(" · ")}
          image={
            mostRecentSoloKill ? (
              <CreatureImage creatureName={mostRecentSoloKill.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />

        {/* ── Compact half-height panels ───────────────────────────── */}
        <StatCard
          label="Assisted Kills"
          value={totalAssisted.toLocaleString()}
          sub={[
            mostRecentAssistedKill?.creature_name,
            timeAgo(mostRecentAssistedKill?.date_last),
          ].filter(Boolean).join(" · ")}
          image={
            mostRecentAssistedKill ? (
              <CreatureImage creatureName={mostRecentAssistedKill.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />
        <StatCard
          label="Highest Value Kill"
          value={highestKilled?.creature_name ?? "None"}
          sub={highestKilled ? [
            `Value: ${highestKilled.creature_value}`,
            timeAgo(highestKilled.date_last_killed),
          ].filter(Boolean).join(" · ") : undefined}
          image={
            highestKilled ? (
              <CreatureImage creatureName={highestKilled.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />
        <StatCard
          label="Most Killed"
          value={mostKilled?.creature_name ?? "None"}
          sub={
            mostKilled
              ? [
                  `${mostKilledTotal.toLocaleString()}×`,
                  timeAgo(mostKilled.date_last),
                ].filter(Boolean).join(" · ")
              : undefined
          }
          image={
            mostKilled ? (
              <CreatureImage creatureName={mostKilled.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />
        <StatCard
          label="Highest Solo Kill"
          value={highestSoloKill?.creature_name ?? "None"}
          sub={
            highestSoloKill
              ? [
                  `Value: ${highestSoloKill.creature_value}`,
                  timeAgo(highestSoloKill.date_last),
                ].filter(Boolean).join(" · ")
              : undefined
          }
          image={
            highestSoloKill ? (
              <CreatureImage creatureName={highestSoloKill.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />
        <StatCard
          label="Most Solo Killed"
          value={mostSoloKilled?.creature_name ?? "None"}
          sub={
            mostSoloKilled
              ? [
                  `${mostSoloKilledTotal.toLocaleString()}×`,
                  mostSoloKilled.date_first ? `first ${timeAgo(mostSoloKilled.date_first)}` : null,
                  mostSoloKilled.date_last ? `last ${timeAgo(mostSoloKilled.date_last)}` : null,
                ].filter(Boolean).join(" · ")
              : undefined
          }
          image={
            mostSoloKilled ? (
              <CreatureImage creatureName={mostSoloKilled.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />

        {highestLootKill && highestLootKill.best_loot_value > 0 && (
          <StatCard
            label="Best Loot Recovery"
            value={highestLootKill.creature_name}
            sub={`${highestLootKill.best_loot_value}c — ${highestLootKill.best_loot_item}`}
            image={<CreatureImage creatureName={highestLootKill.creature_name} className="h-12 w-auto" />}
          />
        )}

        {/* ── Regular stat panels ──────────────────────────────────── */}
        <StatCard
          label="Nemesis"
          value={nemesis?.creature_name ?? "None"}
          sub={nemesis ? `Killed you ${nemesis.killed_by_count} times` : undefined}
          image={
            nemesis ? (
              <CreatureImage creatureName={nemesis.creature_name} className="h-12 w-auto" />
            ) : undefined
          }
        />
        <StatCard
          label="Total Ranks"
          value={totalRanks.toLocaleString()}
          sub={`${trainers.length} trainers`}
        />
        <StatCard
          label="Effective Ranks"
          value={effectiveRounded.toLocaleString()}
          sub={
            totalRanks !== effectiveRounded
              ? `vs ${totalRanks.toLocaleString()} raw`
              : undefined
          }
        />
        {char.untraining_count > 0 && (
          <StatCard label="Untrained" value={`${char.untraining_count}x`} />
        )}
        {chanceOfDepart && (
          <StatCard
            label="Chance of Depart"
            value={`${chanceOfDepart}%`}
            sub={`${char.departs} / ${char.deaths + char.departs}`}
          />
        )}
        {chanceOfChainBreak && (
          <StatCard
            label="Chain Break Rate"
            value={`${chanceOfChainBreak}%`}
            sub={`${char.chains_broken} / ${char.chains_used + char.chains_broken}`}
          />
        )}
        {char.eps_broken > 0 && (
          <StatCard label="EPS Broken" value={char.eps_broken.toLocaleString()} />
        )}

        {/* ── Kill type panels (double height) ─────────────────────── */}
        <KillTypePanel
          label="Vanquishes"
          highest={highestVanquished}
          lowestRecent={lowestRecentVanquished}
          dateField="date_last_vanquished"
        />
        <KillTypePanel
          label="Kills"
          highest={highestKilled}
          lowestRecent={lowestRecentKill}
          dateField="date_last_killed"
        />
        <KillTypePanel
          label="Dispatches"
          highest={highestDispatched}
          lowestRecent={lowestRecentDispatched}
          dateField="date_last_dispatched"
        />
        <KillTypePanel
          label="Slaughters"
          highest={highestSlaughtered}
          lowestRecent={lowestRecentSlaughtered}
          dateField="date_last_slaughtered"
        />
      </div>
    </div>
  );
}
