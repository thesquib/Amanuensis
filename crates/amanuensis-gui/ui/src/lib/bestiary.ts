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
 * unreliable for CV tracking. Demonic Undine (e.g. Ancient Darshak Liche) is NOT
 * excluded — these are specific enemies with reliable, consistent values.
 *
 * Insubstantial Undine was removed from this list: although a few members have
 * population-averaged values (e.g. Ghastly Presence ~650), most are specific named
 * creatures with precise values (e.g. Gho-Wei Ghoulish at 933), so excluding the
 * whole family hid legitimate highest-kills / CV progression.
 */
export const NON_STUFFABLE_FAMILIES = new Set<string>([
  "Ethereal",
]);

/** Returns false for creatures whose bestiary values are unreliable for CV tracking. */
export function isStuffable(name: string): boolean {
  const family = getCreatureFamily(name);
  return family.length > 0 && !NON_STUFFABLE_FAMILIES.has(family);
}

/**
 * URL of the upstream bestiary page listing a family's creatures, or null when no such
 * page exists.
 *
 * The bestiary has no per-creature pages and no per-creature anchors, and its search is
 * AJAX-only with no linkable results URL — so the family page is the only available
 * target. Verified live for 63 of our 66 families, including `AstralElemental.php`, which
 * is missing from the site's own index. Extinct creatures have no family page at all
 * (both "Extinct" and "EXTINCT" 404), hence the null.
 */
export function familyPageUrl(family: string | null | undefined): string | null {
  if (!family) return null;
  const trimmed = family.trim();
  if (!trimmed || trimmed.toUpperCase() === "EXTINCT") return null;
  const slug = trimmed.replace(/\s+/g, "");
  if (!slug) return null;
  return `https://bestiary.clanlord.net/beast/${slug}.php`;
}
