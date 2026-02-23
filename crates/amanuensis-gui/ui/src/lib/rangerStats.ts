import type { Lasty, Trainer } from "../types";
import type { BestiaryEntry } from "./bestiary";

export type StudyStatus = "none" | "in_progress" | "completed" | "abandoned";

export interface StudyState {
  status: StudyStatus;
  message_count: number;
  date: string | null;
}

export interface StudyRecord {
  creature_name: string;
  family: string;
  value: number;
  movements: StudyState;
  befriend: StudyState;
  morph: StudyState;
  duvin_cost: number;
}

export interface FamilyProgress {
  family: string;
  movements_completed: number;
  befriends_completed: number;
  morphs_completed: number;
  bonus_pct: number;
  is_maxed: boolean;
  total_creatures: number;
  representative_creature: string;
}

export interface MorphCandidate {
  creature_name: string;
  family: string;
  value: number;
  current_stage: string;
  duvin_remaining: number;
  atk: number;
  def: number;
  dmg: number;
  hp: number;
  fps: number;
}

export interface RangerStats {
  gossamer_ranks: number;
  duvin_ranks: number;
  total_duvin: number;
  duvin_spent: number;
  duvin_available: number;
  total_studies: number;
  total_befriends: number;
  total_morphs: number;
  studies: StudyRecord[];
  families: FamilyProgress[];
  morph_candidates: MorphCandidate[];
}

const EXCLUDED_FAMILIES = new Set(["--", "Uncategorized"]);

function isTargetEligible(
  name: string,
  bestiaryMap: Record<string, BestiaryEntry>,
): boolean {
  const entry = bestiaryMap[name];
  if (!entry) return false;
  if (EXCLUDED_FAMILIES.has(entry.family)) return false;
  const r = entry.rarity;
  return r.startsWith("Common") || r.startsWith("Medium");
}

const COST_MOVEMENTS = 5;
const COST_BEFRIEND = 10;
const COST_MORPH = 5;

function emptyStudyState(): StudyState {
  return { status: "none", message_count: 0, date: null };
}

function lastyToStudyState(lasty: Lasty): StudyState {
  let status: StudyStatus;
  if (lasty.abandoned_date) {
    status = "abandoned";
  } else if (lasty.finished) {
    status = "completed";
  } else {
    status = "in_progress";
  }
  return {
    status,
    message_count: lasty.message_count,
    date: lasty.completed_date ?? lasty.abandoned_date ?? lasty.last_seen_date,
  };
}

function getTrainerTotalRanks(trainers: Trainer[], name: string): number {
  let total = 0;
  for (const t of trainers) {
    if (t.trainer_name === name) {
      total += t.ranks + t.modified_ranks;
    }
  }
  return total;
}

export function computeRangerStats(
  lastys: Lasty[],
  trainers: Trainer[],
  creatureValues: Record<string, number>,
  bestiaryMap: Record<string, BestiaryEntry>,
): RangerStats {
  const gossamer_ranks = getTrainerTotalRanks(trainers, "Gossamer");
  const duvin_ranks = getTrainerTotalRanks(trainers, "Duvin Beastlore");
  const total_duvin = duvin_ranks + Math.floor(gossamer_ranks / 4);

  // Group lastys by creature_name
  const creatureLastys = new Map<string, Map<string, Lasty>>();
  for (const l of lastys) {
    if (!creatureLastys.has(l.creature_name)) {
      creatureLastys.set(l.creature_name, new Map());
    }
    creatureLastys.get(l.creature_name)!.set(l.lasty_type, l);
  }

  // Build family completion tracking for discount calculation
  // Track completed studies per family per type, ordered by completion date
  const familyCompletedMovements = new Map<string, { creature: string; date: string | null }[]>();
  const familyCompletedBefriends = new Map<string, { creature: string; date: string | null }[]>();
  const familyCompletedMorphs = new Map<string, { creature: string; date: string | null }[]>();

  for (const [creature, typeMap] of creatureLastys) {
    const family = bestiaryMap[creature]?.family ?? "";
    if (!family) continue;

    for (const [type, lasty] of typeMap) {
      if (!lasty.finished) continue;
      const entry = { creature, date: lasty.completed_date };
      if (type === "Movements") {
        if (!familyCompletedMovements.has(family)) familyCompletedMovements.set(family, []);
        familyCompletedMovements.get(family)!.push(entry);
      } else if (type === "Befriend") {
        if (!familyCompletedBefriends.has(family)) familyCompletedBefriends.set(family, []);
        familyCompletedBefriends.get(family)!.push(entry);
      } else if (type === "Morph") {
        if (!familyCompletedMorphs.has(family)) familyCompletedMorphs.set(family, []);
        familyCompletedMorphs.get(family)!.push(entry);
      }
    }
  }

  // Sort each family's completions by date for discount ordering
  const dateSorter = (a: { date: string | null }, b: { date: string | null }) => {
    if (!a.date && !b.date) return 0;
    if (!a.date) return -1;
    if (!b.date) return 1;
    return a.date.localeCompare(b.date);
  };
  for (const arr of familyCompletedMovements.values()) arr.sort(dateSorter);
  for (const arr of familyCompletedBefriends.values()) arr.sort(dateSorter);
  for (const arr of familyCompletedMorphs.values()) arr.sort(dateSorter);

  // Compute per-creature cost with family discount
  function computeCostForStudy(
    creature: string,
    studyType: "Movements" | "Befriend" | "Morph",
    baseCost: number,
    isActive: boolean,
  ): number {
    if (!isActive) return 0;
    const family = bestiaryMap[creature]?.family ?? "";
    if (!family) return baseCost;

    let completedList: { creature: string; date: string | null }[];
    if (studyType === "Movements") {
      completedList = familyCompletedMovements.get(family) ?? [];
    } else if (studyType === "Befriend") {
      completedList = familyCompletedBefriends.get(family) ?? [];
    } else {
      completedList = familyCompletedMorphs.get(family) ?? [];
    }

    // Find the index of this creature in the completion order
    const idx = completedList.findIndex((c) => c.creature === creature);
    const priorCount = idx >= 0 ? idx : completedList.length;
    return Math.max(1, baseCost - priorCount);
  }

  // Build study records
  let duvin_spent = 0;
  let total_studies = 0;
  let total_befriends = 0;
  let total_morphs = 0;
  const studies: StudyRecord[] = [];

  for (const [creature, typeMap] of creatureLastys) {
    const family = bestiaryMap[creature]?.family ?? "";
    const value = creatureValues[creature] ?? 0;

    const movLasty = typeMap.get("Movements");
    const befLasty = typeMap.get("Befriend");
    const morLasty = typeMap.get("Morph");

    const movements = movLasty ? lastyToStudyState(movLasty) : emptyStudyState();
    const befriend = befLasty ? lastyToStudyState(befLasty) : emptyStudyState();
    const morph = morLasty ? lastyToStudyState(morLasty) : emptyStudyState();

    let cost = 0;
    if (movements.status === "completed" || movements.status === "in_progress") {
      cost += computeCostForStudy(creature, "Movements", COST_MOVEMENTS, true);
    }
    if (befriend.status === "completed" || befriend.status === "in_progress") {
      cost += computeCostForStudy(creature, "Befriend", COST_BEFRIEND, true);
    }
    if (morph.status === "completed" || morph.status === "in_progress") {
      cost += computeCostForStudy(creature, "Morph", COST_MORPH, true);
    }

    duvin_spent += cost;

    if (movements.status === "completed") total_studies++;
    if (befriend.status === "completed") total_befriends++;
    if (morph.status === "completed") total_morphs++;

    studies.push({
      creature_name: creature,
      family,
      value,
      movements,
      befriend,
      morph,
      duvin_cost: cost,
    });
  }

  // Sort studies by value desc
  studies.sort((a, b) => b.value - a.value || a.creature_name.localeCompare(b.creature_name));

  // Build family progress
  const familyMap = new Map<string, FamilyProgress>();
  // Gather all families from bestiary
  const allBestiaryFamilies = new Map<string, { count: number; representative: string }>();
  for (const [name, entry] of Object.entries(bestiaryMap)) {
    if (!entry.family) continue;
    const existing = allBestiaryFamilies.get(entry.family);
    if (!existing) {
      allBestiaryFamilies.set(entry.family, { count: 1, representative: name });
    } else {
      existing.count++;
    }
  }

  // Initialize family progress from completed studies
  for (const study of studies) {
    if (!study.family) continue;
    if (!familyMap.has(study.family)) {
      const bestiaryInfo = allBestiaryFamilies.get(study.family);
      familyMap.set(study.family, {
        family: study.family,
        movements_completed: 0,
        befriends_completed: 0,
        morphs_completed: 0,
        bonus_pct: 0,
        is_maxed: false,
        total_creatures: bestiaryInfo?.count ?? 0,
        representative_creature: bestiaryInfo?.representative ?? study.creature_name,
      });
    }
    const fp = familyMap.get(study.family)!;
    if (study.movements.status === "completed") fp.movements_completed++;
    if (study.befriend.status === "completed") fp.befriends_completed++;
    if (study.morph.status === "completed") fp.morphs_completed++;
  }

  // Compute bonus percentages
  for (const fp of familyMap.values()) {
    fp.bonus_pct = Math.min(50, fp.movements_completed * 10);
    fp.is_maxed = fp.bonus_pct >= 50;
  }

  const families = Array.from(familyMap.values()).sort((a, b) =>
    b.bonus_pct - a.bonus_pct || a.family.localeCompare(b.family),
  );

  // Build morph candidates: creatures with value > 0 not yet morph-completed
  const studiedCreatures = new Set(creatureLastys.keys());
  const morph_candidates: MorphCandidate[] = [];

  // First: creatures already being studied but not morph-completed
  for (const study of studies) {
    if (study.value <= 0) continue;
    if (study.morph.status === "completed") continue;
    if (!isTargetEligible(study.creature_name, bestiaryMap)) continue;

    let current_stage: string;
    let duvin_remaining = 0;
    const family = study.family;

    if (study.befriend.status === "completed") {
      current_stage = "Befriend";
      // Need morph cost
      const morphCost = computeCostForStudy(study.creature_name, "Morph", COST_MORPH, true);
      duvin_remaining = morphCost;
    } else if (study.movements.status === "completed") {
      current_stage = "Movements";
      const befCost = computeCostForStudy(study.creature_name, "Befriend", COST_BEFRIEND, true);
      const morphCost = computeCostForStudy(study.creature_name, "Morph", COST_MORPH, true);
      duvin_remaining = befCost + morphCost;
    } else if (study.movements.status === "in_progress") {
      current_stage = "Studying";
      const movCost = computeCostForStudy(study.creature_name, "Movements", COST_MOVEMENTS, true);
      const befCost = computeCostForStudy(study.creature_name, "Befriend", COST_BEFRIEND, true);
      const morphCost = computeCostForStudy(study.creature_name, "Morph", COST_MORPH, true);
      duvin_remaining = movCost + befCost + morphCost;
    } else {
      current_stage = "None";
      const movCost = family ? Math.max(1, COST_MOVEMENTS - (familyCompletedMovements.get(family)?.length ?? 0)) : COST_MOVEMENTS;
      const befCost = family ? Math.max(1, COST_BEFRIEND - (familyCompletedBefriends.get(family)?.length ?? 0)) : COST_BEFRIEND;
      const morphCost = family ? Math.max(1, COST_MORPH - (familyCompletedMorphs.get(family)?.length ?? 0)) : COST_MORPH;
      duvin_remaining = movCost + befCost + morphCost;
    }

    const be = bestiaryMap[study.creature_name];
    morph_candidates.push({
      creature_name: study.creature_name,
      family: study.family,
      value: study.value,
      current_stage,
      duvin_remaining,
      atk: be?.atk ?? 0,
      def: be?.def ?? 0,
      dmg: be?.dmg ?? 0,
      hp: be?.hp ?? 0,
      fps: be?.fps ?? 0,
    });
  }

  // Then: unstudied creatures from creature_values
  for (const [name, value] of Object.entries(creatureValues)) {
    if (value <= 0 || studiedCreatures.has(name)) continue;
    if (!isTargetEligible(name, bestiaryMap)) continue;
    const family = bestiaryMap[name]?.family ?? "";
    const movCost = family ? Math.max(1, COST_MOVEMENTS - (familyCompletedMovements.get(family)?.length ?? 0)) : COST_MOVEMENTS;
    const befCost = family ? Math.max(1, COST_BEFRIEND - (familyCompletedBefriends.get(family)?.length ?? 0)) : COST_BEFRIEND;
    const morphCost = family ? Math.max(1, COST_MORPH - (familyCompletedMorphs.get(family)?.length ?? 0)) : COST_MORPH;
    const be = bestiaryMap[name];

    morph_candidates.push({
      creature_name: name,
      family,
      value,
      current_stage: "None",
      duvin_remaining: movCost + befCost + morphCost,
      atk: be?.atk ?? 0,
      def: be?.def ?? 0,
      dmg: be?.dmg ?? 0,
      hp: be?.hp ?? 0,
      fps: be?.fps ?? 0,
    });
  }

  // Sort by value desc (default ordering; UI applies per-category sorting and truncation)
  morph_candidates.sort((a, b) => b.value - a.value || a.creature_name.localeCompare(b.creature_name));

  return {
    gossamer_ranks,
    duvin_ranks,
    total_duvin,
    duvin_spent,
    duvin_available: total_duvin - duvin_spent,
    total_studies,
    total_befriends,
    total_morphs,
    studies,
    families,
    morph_candidates,
  };
}
