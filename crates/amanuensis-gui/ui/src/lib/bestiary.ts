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
