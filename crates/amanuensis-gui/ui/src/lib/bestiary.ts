import bestiaryData from "../../../data/bestiary_images.json";

export interface BestiaryEntry {
  family: string;
  rarity: string;
  pic: string;
  w: number;
  h: number;
  atk: number;
  def: number;
  dmg: number;
  hp: number;
  fps: number;
}

export const bestiaryMap = bestiaryData as Record<string, BestiaryEntry>;

export function getCreatureImageUrl(name: string): string | null {
  const lookupName = name.startsWith("Captured ") ? name.slice("Captured ".length) : name;
  const entry = bestiaryMap[lookupName];
  if (!entry) return null;
  return `/bestiary/${entry.pic}`;
}

export function getCreatureFamily(name: string): string {
  return bestiaryMap[name]?.family ?? "";
}

export const NON_STUFFABLE_FAMILIES = new Set([
  "Ethereal",
  "Insubstantial Undine",
  "Substantial Undine",
  "Skeletal Undine",
  "Demonic Undine",
]);

/** Returns false for ethereal/undine creatures whose bestiary values are unreliable for CV tracking. */
export function isStuffable(name: string): boolean {
  const family = bestiaryMap[name]?.family ?? "";
  return !NON_STUFFABLE_FAMILIES.has(family);
}
