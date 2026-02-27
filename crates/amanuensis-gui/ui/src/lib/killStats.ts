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
  mostKilled: Kill | null;
  mostKilledTotal: number;
  highestSoloKill: Kill | null;
  mostSoloKilled: Kill | null;
  mostSoloKilledTotal: number;
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

    const mostKilledTotal = mostKilled ? totalKillCount(mostKilled) : 0;
    if (total > mostKilledTotal) mostKilled = k;

    if (solo > 0 && k.creature_value > (highestSoloKill?.creature_value ?? 0)) highestSoloKill = k;

    const bestSolo = mostSoloKilled ? soloKillCount(mostSoloKilled) : 0;
    if (solo > bestSolo) mostSoloKilled = k;
  }

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
    mostKilled,
    mostKilledTotal: mostKilled ? totalKillCount(mostKilled) : 0,
    highestSoloKill,
    mostSoloKilled,
    mostSoloKilledTotal: mostSoloKilled ? soloKillCount(mostSoloKilled) : 0,
  };
}
