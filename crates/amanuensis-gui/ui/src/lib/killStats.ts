import type { Kill } from "../types";

/** Total solo kills (killed + slaughtered + vanquished + dispatched). */
export function soloKillCount(k: Kill): number {
  return k.killed_count + k.slaughtered_count + k.vanquished_count + k.dispatched_count;
}

/** Total assisted kills. */
export function assistedKillCount(k: Kill): number {
  return (
    k.assisted_kill_count +
    k.assisted_slaughter_count +
    k.assisted_vanquish_count +
    k.assisted_dispatch_count
  );
}

/** Total of all kill types (solo + assisted). */
export function totalKillCount(k: Kill): number {
  return soloKillCount(k) + assistedKillCount(k);
}

export interface KillStats {
  totalSolo: number;
  totalAssisted: number;
  totalKilledBy: number;
  totalVanquished: number;
  totalSlaughtered: number;
  totalKilled: number;
  totalDispatched: number;
  uniqueCreatures: number;
  nemesis: Kill | null;
  highestKill: Kill | null;
  highestKilled: Kill | null;
  highestSlaughtered: Kill | null;
  highestVanquished: Kill | null;
  highestDispatched: Kill | null;
  lowestRecentKill: Kill | null;
  lowestRecentSlaughtered: Kill | null;
  lowestRecentVanquished: Kill | null;
  lowestRecentDispatched: Kill | null;
  mostKilled: Kill | null;
  mostKilledTotal: number;
  highestSoloKill: Kill | null;
  mostSoloKilled: Kill | null;
  mostSoloKilledTotal: number;
  mostRecentSoloKill: Kill | null;
  mostRecentAssistedKill: Kill | null;
}

/** Returns the lowest-value creature among the 10 most recently killed by a specific type. */
function lowestRecentByType(
  kills: Kill[],
  dateField: keyof Kill,
  countField: keyof Kill,
): Kill | null {
  const eligible = kills.filter(
    (k) => (k[countField] as number) > 0 && k[dateField] !== null,
  );
  eligible.sort((a, b) =>
    ((b[dateField] as string) ?? "").localeCompare((a[dateField] as string) ?? ""),
  );
  const recent = eligible.slice(0, 10);
  if (recent.length === 0) return null;
  return recent.reduce((min, k) => (k.creature_value < min.creature_value ? k : min));
}

export function computeKillStats(kills: Kill[]): KillStats {
  let totalSolo = 0;
  let totalAssisted = 0;
  let totalKilledBy = 0;
  let totalVanquished = 0;
  let totalSlaughtered = 0;
  let totalKilled = 0;
  let totalDispatched = 0;
  let nemesis: Kill | null = null;
  let highestKill: Kill | null = null;
  let highestKilled: Kill | null = null;
  let highestSlaughtered: Kill | null = null;
  let highestVanquished: Kill | null = null;
  let highestDispatched: Kill | null = null;
  let mostKilled: Kill | null = null;
  let highestSoloKill: Kill | null = null;
  let mostSoloKilled: Kill | null = null;

  for (const k of kills) {
    const solo = soloKillCount(k);
    const assisted = assistedKillCount(k);
    const total = solo + assisted;

    totalSolo += solo;
    totalAssisted += assisted;
    totalKilledBy += k.killed_by_count;
    totalVanquished += k.vanquished_count + k.assisted_vanquish_count;
    totalSlaughtered += k.slaughtered_count + k.assisted_slaughter_count;
    totalKilled += k.killed_count + k.assisted_kill_count;
    totalDispatched += k.dispatched_count + k.assisted_dispatch_count;

    if (k.killed_by_count > (nemesis?.killed_by_count ?? 0)) nemesis = k;

    if (total > 0 && k.creature_value > (highestKill?.creature_value ?? 0)) highestKill = k;

    if (k.killed_count > 0 && k.creature_value > (highestKilled?.creature_value ?? 0)) highestKilled = k;
    if (k.slaughtered_count > 0 && k.creature_value > (highestSlaughtered?.creature_value ?? 0)) highestSlaughtered = k;
    if (k.vanquished_count > 0 && k.creature_value > (highestVanquished?.creature_value ?? 0)) highestVanquished = k;
    if (k.dispatched_count > 0 && k.creature_value > (highestDispatched?.creature_value ?? 0)) highestDispatched = k;

    const mostKilledTotal = mostKilled ? totalKillCount(mostKilled) : 0;
    if (total > mostKilledTotal) mostKilled = k;

    if (solo > 0 && k.creature_value > (highestSoloKill?.creature_value ?? 0)) highestSoloKill = k;

    const bestSolo = mostSoloKilled ? soloKillCount(mostSoloKilled) : 0;
    if (solo > bestSolo) mostSoloKilled = k;
  }

  // Lowest value among the 10 most recently encountered creatures (any solo kill type)
  const recentAny = kills
    .filter((k) => soloKillCount(k) > 0 && k.date_last !== null)
    .sort((a, b) => ((b.date_last ?? "").localeCompare(a.date_last ?? "")))
    .slice(0, 10);
  const lowestRecentKill =
    recentAny.length > 0
      ? recentAny.reduce((min, k) => (k.creature_value < min.creature_value ? k : min))
      : null;

  const lowestRecentSlaughtered = lowestRecentByType(kills, "date_last_slaughtered", "slaughtered_count");
  const lowestRecentVanquished  = lowestRecentByType(kills, "date_last_vanquished",  "vanquished_count");
  const lowestRecentDispatched  = lowestRecentByType(kills, "date_last_dispatched",  "dispatched_count");

  const mostRecentSoloKill = kills
    .filter((k) => soloKillCount(k) > 0 && k.date_last !== null)
    .reduce<Kill | null>((best, k) => {
      if (!best) return k;
      return (k.date_last ?? "") > (best.date_last ?? "") ? k : best;
    }, null);

  const mostRecentAssistedKill = kills
    .filter((k) => assistedKillCount(k) > 0 && k.date_last !== null)
    .reduce<Kill | null>((best, k) => {
      if (!best) return k;
      return (k.date_last ?? "") > (best.date_last ?? "") ? k : best;
    }, null);

  return {
    totalSolo,
    totalAssisted,
    totalKilledBy,
    totalVanquished,
    totalSlaughtered,
    totalKilled,
    totalDispatched,
    uniqueCreatures: kills.length,
    nemesis,
    highestKill,
    highestKilled,
    highestSlaughtered,
    highestVanquished,
    highestDispatched,
    lowestRecentKill,
    lowestRecentSlaughtered,
    lowestRecentVanquished,
    lowestRecentDispatched,
    mostKilled,
    mostKilledTotal: mostKilled ? totalKillCount(mostKilled) : 0,
    highestSoloKill,
    mostSoloKilled,
    mostSoloKilledTotal: mostSoloKilled ? soloKillCount(mostSoloKilled) : 0,
    mostRecentSoloKill,
    mostRecentAssistedKill,
  };
}
