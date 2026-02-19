/**
 * Fighter stat calculations based on Gorvin's Fighter Calculator (dps.html).
 * Assumes Human race, Roguewood Club, no items (base fighter).
 */

// Human race base stats
const RACE_BASE = {
  accuracy: 300,
  minDamage: 100,
  maxDamage: 200,
  balance: 5000,
  balRegen: 400,
  health: 3000,
  defense: 300,
  healthRegen: 100,
  spirit: 800,
  spiritRegen: 600,
};

// Slaughter point costs per trainer
const SP_COSTS: Record<string, number> = {
  Atkus: 21,
  Darkus: 19,
  Balthus: 18,
  Regia: 18,
  Evus: 24,
  Swengus: 18,
  Histia: 29,
  Detha: 22,
  Bodrus: 24,
  Hardia: 30,
  Troilus: 20,
  Spiritus: 20,
  Aktur: 22,
  Atkia: 21,
  Darktur: 20,
  Angilsa: 10,
  Knox: 12,
  Heen: 20,
  Bangus: 23,
  Farly: 22,
  Stedfustus: 25,
  Forvyola: 23,
  Anemia: 24,
  Rodnus: 20,
  Erthron: 29,
};

// Human race SP: 300+100+200+floor(5000/3)+400+floor(3000/3)+300+100+800+600
const RACE_SP = 300 + 100 + 200 + Math.floor(5000 / 3) + 400 + Math.floor(3000 / 3) + 300 + 100 + 800 + 600;

export interface FighterStats {
  // Raw computed stats (before adding race base)
  accuracy: number;
  minDamage: number;
  maxDamage: number;
  balance: number;
  balRegen: number;
  health: number;
  defense: number;
  healthRegen: number;
  spirit: number;
  spiritRegen: number;
  healReceptivity: number;

  // Derived stats
  damageMin: number;
  damageMax: number;
  balancePerSwing: number;
  shieldstoneDrain: number;
  healthPerFrame: number;
  balancePerFrame: number;
  spiritPerFrame: number;

  // Aggregate
  trainedRanks: number;
  effectiveRanks: number;
  slaughterPoints: number;
}

function r(ranks: Map<string, number>, name: string): number {
  return ranks.get(name) ?? 0;
}

export function computeFighterStats(
  ranks: Map<string, number>,
  multipliers: Map<string, number>,
): FighterStats {
  // Trainer rank helper
  const atkus = r(ranks, "Atkus");
  const darkus = r(ranks, "Darkus");
  const balthus = r(ranks, "Balthus");
  const regia = r(ranks, "Regia");
  const evus = r(ranks, "Evus");
  const swengus = r(ranks, "Swengus");
  const histia = r(ranks, "Histia");
  const detha = r(ranks, "Detha");
  const bodrus = r(ranks, "Bodrus");
  const hardia = r(ranks, "Hardia");
  const troilus = r(ranks, "Troilus");
  const spiritus = r(ranks, "Spiritus");
  const aktur = r(ranks, "Aktur");
  const atkia = r(ranks, "Atkia");
  const darktur = r(ranks, "Darktur");
  const angilsa = r(ranks, "Angilsa");
  const knox = r(ranks, "Knox");
  const heen = r(ranks, "Heen");
  const bangus = r(ranks, "Bangus");
  const farly = r(ranks, "Farly");
  const stedfustus = r(ranks, "Stedfustus");
  const forvyola = r(ranks, "Forvyola");
  const anemia = r(ranks, "Anemia");
  const rodnus = r(ranks, "Rodnus");
  const erthron = r(ranks, "Erthron");

  // Primary stat formulas (base fighter, no subclass)
  const accuracy =
    atkus * 16 + evus * 4 + bodrus * 4 + aktur * 25 + atkia * 13 -
    knox * 4 - angilsa * 4 + bangus * 2 + erthron * 3;

  const minDamage =
    darkus * 6 + evus * 1 + bodrus * 1 + knox * 11 - angilsa * 1 +
    erthron * 1 + atkia * 3 + darktur * 10 + bangus * 2;

  const maxDamage =
    darkus * 6 + evus * 1 + bodrus * 1 + knox * 11 - angilsa * 1 +
    erthron * 1 + atkia * 3 + darktur * 10 + bangus * 3 + hardia * 1;

  const balance =
    balthus * 51 + evus * 18 + bodrus * 9 + atkus * 15 + darkus * 18 +
    swengus * 30 + knox * 18 - angilsa * 18 + bangus * 21 + erthron * 15;

  const balRegen =
    regia * 15 + evus * 4 + bodrus * 3 + atkus * 1 + darkus * 1 +
    swengus * 7 - knox * 2 + angilsa * 26 + forvyola * 8 + bangus * 5 +
    erthron * 3 + atkia * 3 + stedfustus * 6 + anemia * 8;

  const health =
    histia * 111 + evus * 24 + bodrus * 24 + detha * 3 + rodnus * 36 +
    farly * 48 - knox * 24 - angilsa * 24 + forvyola * 54 + bangus * 6 +
    erthron * 24 + spiritus * 21 + stedfustus * 54 + anemia * 69;

  const defense =
    detha * 19 + evus * 1 + bodrus * 1 + hardia * 1 + farly * 2 -
    knox * 1 - angilsa * 1 + erthron * 7;

  const healthRegen =
    troilus * 6 + farly * 4 + bangus * 1 + stedfustus * 1 - anemia * 1;

  const spirit = spiritus * 9;

  const spiritRegen = 0; // Base fighter has no spirit regen trainers

  const healReceptivity = 2 * rodnus + spiritus;

  // Total stats (trainer contribution + race base)
  const totalAccuracy = accuracy + RACE_BASE.accuracy;
  const totalMinDmg = minDamage + RACE_BASE.minDamage;
  const totalMaxDmg = maxDamage + RACE_BASE.maxDamage;
  const totalBalance = balance + RACE_BASE.balance;
  const totalBalRegen = balRegen + RACE_BASE.balRegen;
  const totalHealth = health + RACE_BASE.health;
  const totalDefense = defense + RACE_BASE.defense;
  const totalHealthRegen = healthRegen + RACE_BASE.healthRegen;
  const totalSpirit = spirit + RACE_BASE.spirit;
  const totalSpiritRegen = spiritRegen + RACE_BASE.spiritRegen;

  // Derived stats
  const damageMin = Math.max(totalMinDmg, 0) + 100;
  const damageMax = Math.max(totalMaxDmg * 3, 0) + 100;

  const offense = totalAccuracy + (3 * totalMaxDmg + totalMinDmg) / 4;
  const balancePerSwing = Math.floor((5 / 3) * Math.max(offense, 200));

  // Shieldstone drain
  let shieldstoneDrain: number;
  if (heen < 50) {
    shieldstoneDrain = Math.round(1066 - (436 * heen) / 49);
  } else {
    shieldstoneDrain = Math.round((628 * 50) / heen);
  }

  const healthPerFrame = Math.floor(totalHealthRegen) / 100;
  const balancePerFrame = totalBalRegen / 6;
  const spiritPerFrame = Math.floor(totalSpiritRegen) / 100;

  // Trained ranks (sum of all ranks)
  let trainedRanks = 0;
  for (const total of ranks.values()) {
    trainedRanks += total;
  }

  // Effective ranks (ranks × multiplier)
  let effectiveRanks = 0;
  for (const [name, total] of ranks) {
    effectiveRanks += total * (multipliers.get(name) ?? 1.0);
  }
  effectiveRanks = Math.round(effectiveRanks * 10) / 10;

  // Slaughter points
  let slaughterPoints = RACE_SP;
  for (const [name, total] of ranks) {
    const cost = SP_COSTS[name];
    if (cost) {
      slaughterPoints += total * cost;
    }
  }

  return {
    accuracy: totalAccuracy,
    minDamage: totalMinDmg,
    maxDamage: totalMaxDmg,
    balance: totalBalance,
    balRegen: totalBalRegen,
    health: totalHealth,
    defense: totalDefense,
    healthRegen: totalHealthRegen,
    spirit: totalSpirit,
    spiritRegen: totalSpiritRegen,
    healReceptivity,
    damageMin,
    damageMax,
    balancePerSwing,
    shieldstoneDrain,
    healthPerFrame,
    balancePerFrame,
    spiritPerFrame,
    trainedRanks,
    effectiveRanks,
    slaughterPoints,
  };
}
