import bestiaryData from "../../../data/bestiary_images.json";

const bestiaryMap = bestiaryData as Record<string, { family: string; pic: string; w: number; h: number }>;

export function getCreatureImageUrl(name: string): string | null {
  const entry = bestiaryMap[name];
  if (!entry) return null;
  return `/bestiary/${entry.pic}`;
}
