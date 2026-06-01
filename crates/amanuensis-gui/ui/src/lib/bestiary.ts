import { useStore } from "./store";
import type { BestiaryEntry } from "../types";

export type { BestiaryEntry } from "../types";

/** Returns a snapshot of the entire bestiary name -> entry map. */
export function getBestiaryMap(): Record<string, BestiaryEntry> {
  return useStore.getState().bestiaryByName;
}

/** Look up a creature by exact name from the loaded bestiary. */
export function getBestiaryEntry(name: string): BestiaryEntry | undefined {
  return useStore.getState().bestiaryByName[name];
}

/** Resolve a sprite URL relative to the public/bestiary folder. */
export function getCreatureImageUrl(name: string): string | null {
  const lookupName = name.startsWith("Captured ")
    ? name.slice("Captured ".length)
    : name;
  const entry = getBestiaryEntry(lookupName);
  return entry?.static_pic ? `/bestiary/${entry.static_pic}` : null;
}

/** Convenience: family of the creature, or "" if not in the bestiary. */
export function getCreatureFamily(name: string): string {
  return getBestiaryEntry(name)?.family ?? "";
}

/**
 * Families excluded from coin-level and CV graph because their bestiary values are
 * averaged across multiple population strengths (e.g. Ghastly Presence appears in
 * weak and strong variants, averaged to ~650). Demonic Undine (e.g. Ancient Darshak
 * Liche) is NOT excluded — these are specific enemies with reliable, consistent values.
 */
export const NON_STUFFABLE_FAMILIES = new Set<string>([
  "Ethereal",
  "Insubstantial Undine",
]);

/** Returns false for creatures whose bestiary values are unreliable for CV tracking. */
export function isStuffable(name: string): boolean {
  const family = getCreatureFamily(name);
  return family.length > 0 && !NON_STUFFABLE_FAMILIES.has(family);
}
