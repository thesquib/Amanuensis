/**
 * Extract the date portion from a Clan Lord timestamp string.
 * Timestamps are stored as "M/D/YY H:MM:SSa/p"; this returns "M/D/YY".
 */
export function formatDate(val: string | null | undefined): string {
  return val ? val.split(" ")[0] : "";
}

/**
 * Return today's date as a Clan Lord–format string: "M/D/YY".
 */
export function todayMDYY(): string {
  const now = new Date();
  const m = now.getMonth() + 1;
  const d = now.getDate();
  const yy = String(now.getFullYear()).slice(2);
  return `${m}/${d}/${yy}`;
}
