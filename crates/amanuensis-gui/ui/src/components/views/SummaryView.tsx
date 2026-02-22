import { useState, useEffect, useMemo, useCallback } from "react";
import { useStore } from "../../lib/store";
import { StatCard } from "../shared/StatCard";
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

  const totalKills = kills.reduce(
    (sum, k) =>
      sum +
      k.killed_count +
      k.slaughtered_count +
      k.vanquished_count +
      k.dispatched_count,
    0,
  );
  const totalAssisted = kills.reduce(
    (sum, k) =>
      sum +
      k.assisted_kill_count +
      k.assisted_slaughter_count +
      k.assisted_vanquish_count +
      k.assisted_dispatch_count,
    0,
  );
  const uniqueCreatures = kills.length;

  // Find nemesis (most killed-by)
  const nemesis = kills.reduce(
    (best, k) => (k.killed_by_count > (best?.killed_by_count ?? 0) ? k : best),
    null as (typeof kills)[0] | null,
  );

  // Find highest value creature killed (solo + assisted)
  const highestKill = kills.reduce(
    (best, k) => {
      const total = k.killed_count + k.slaughtered_count + k.vanquished_count + k.dispatched_count +
        k.assisted_kill_count + k.assisted_slaughter_count + k.assisted_vanquish_count + k.assisted_dispatch_count;
      if (total === 0) return best;
      return k.creature_value > (best?.creature_value ?? 0) ? k : best;
    },
    null as (typeof kills)[0] | null,
  );

  // Find most killed creature (solo + assisted)
  const mostKilled = kills.reduce(
    (best, k) => {
      const total = k.killed_count + k.slaughtered_count + k.vanquished_count + k.dispatched_count +
        k.assisted_kill_count + k.assisted_slaughter_count + k.assisted_vanquish_count + k.assisted_dispatch_count;
      const bestTotal = best
        ? best.killed_count + best.slaughtered_count + best.vanquished_count + best.dispatched_count +
          best.assisted_kill_count + best.assisted_slaughter_count + best.assisted_vanquish_count + best.assisted_dispatch_count
        : 0;
      return total > bestTotal ? k : best;
    },
    null as (typeof kills)[0] | null,
  );
  const mostKilledTotal = mostKilled
    ? mostKilled.killed_count + mostKilled.slaughtered_count + mostKilled.vanquished_count + mostKilled.dispatched_count +
      mostKilled.assisted_kill_count + mostKilled.assisted_slaughter_count + mostKilled.assisted_vanquish_count + mostKilled.assisted_dispatch_count
    : 0;

  // Find highest value creature solo-killed
  const highestSoloKill = kills.reduce(
    (best, k) => {
      const solo = k.killed_count + k.slaughtered_count + k.vanquished_count + k.dispatched_count;
      if (solo === 0) return best;
      return k.creature_value > (best?.creature_value ?? 0) ? k : best;
    },
    null as (typeof kills)[0] | null,
  );

  // Find most solo-killed creature
  const mostSoloKilled = kills.reduce(
    (best, k) => {
      const solo = k.killed_count + k.slaughtered_count + k.vanquished_count + k.dispatched_count;
      const bestSolo = best
        ? best.killed_count + best.slaughtered_count + best.vanquished_count + best.dispatched_count
        : 0;
      return solo > bestSolo ? k : best;
    },
    null as (typeof kills)[0] | null,
  );
  const mostSoloKilledTotal = mostSoloKilled
    ? mostSoloKilled.killed_count + mostSoloKilled.slaughtered_count + mostSoloKilled.vanquished_count + mostSoloKilled.dispatched_count
    : 0;

  const totalRanks = trainers.reduce(
    (sum, t) => sum + t.ranks + t.modified_ranks,
    0,
  );

  const effectiveRanks = useMemo(() => {
    const multMap = new Map<string, number>();
    for (const t of trainerDb) {
      multMap.set(t.name, t.multiplier);
    }
    return trainers.reduce(
      (sum, t) =>
        sum + (t.ranks + t.modified_ranks) * (multMap.get(t.trainer_name) ?? 1.0),
      0,
    );
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
        <div className="row-span-2 flex items-center justify-center rounded-lg bg-[var(--color-card)] p-4">
          <CharacterPortrait
            name={char.name}
            className="h-full max-h-40 w-auto rounded-lg"
          />
        </div>
        <StatCard label="Coin Level" value={char.coin_level.toLocaleString()} />
        <StatCard label="Logins" value={char.logins.toLocaleString()} />
        <StatCard label="Deaths" value={char.deaths.toLocaleString()} />
        <StatCard label="Departs" value={char.departs.toLocaleString()} />
        <StatCard
          label="Solo Kills"
          value={totalKills.toLocaleString()}
          sub={`${uniqueCreatures} unique creatures`}
        />
        <StatCard
          label="Assisted Kills"
          value={totalAssisted.toLocaleString()}
        />
        <StatCard
          label="Highest Value Kill"
          value={highestKill?.creature_name ?? "None"}
          sub={
            highestKill
              ? `Value: ${highestKill.creature_value}`
              : undefined
          }
          image={
            highestKill ? (
              <CreatureImage
                creatureName={highestKill.creature_name}
                className="h-12 w-auto"
              />
            ) : undefined
          }
        />
        <StatCard
          label="Most Killed"
          value={mostKilled?.creature_name ?? "None"}
          sub={
            mostKilled
              ? `${mostKilledTotal.toLocaleString()} times`
              : undefined
          }
          image={
            mostKilled ? (
              <CreatureImage
                creatureName={mostKilled.creature_name}
                className="h-12 w-auto"
              />
            ) : undefined
          }
        />
        <StatCard
          label="Highest Solo Kill"
          value={highestSoloKill?.creature_name ?? "None"}
          sub={
            highestSoloKill
              ? `Value: ${highestSoloKill.creature_value}`
              : undefined
          }
          image={
            highestSoloKill ? (
              <CreatureImage
                creatureName={highestSoloKill.creature_name}
                className="h-12 w-auto"
              />
            ) : undefined
          }
        />
        <StatCard
          label="Most Solo Killed"
          value={mostSoloKilled?.creature_name ?? "None"}
          sub={
            mostSoloKilled
              ? `${mostSoloKilledTotal.toLocaleString()} times`
              : undefined
          }
          image={
            mostSoloKilled ? (
              <CreatureImage
                creatureName={mostSoloKilled.creature_name}
                className="h-12 w-auto"
              />
            ) : undefined
          }
        />
        <StatCard
          label="Nemesis"
          value={nemesis?.creature_name ?? "None"}
          sub={
            nemesis ? `Killed you ${nemesis.killed_by_count} times` : undefined
          }
          image={
            nemesis ? (
              <CreatureImage
                creatureName={nemesis.creature_name}
                className="h-12 w-auto"
              />
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
          <StatCard
            label="Untrained"
            value={`${char.untraining_count}x`}
          />
        )}
        <StatCard
          label="Good Karma"
          value={char.good_karma.toLocaleString()}
        />
        <StatCard
          label="Bad Karma"
          value={char.bad_karma.toLocaleString()}
        />
        <StatCard
          label="Esteem"
          value={char.esteem.toLocaleString()}
        />
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
          <StatCard
            label="EPS Broken"
            value={char.eps_broken.toLocaleString()}
          />
        )}
      </div>
    </div>
  );
}
