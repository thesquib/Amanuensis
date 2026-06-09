/**
 * Extract the date portion from a Clan Lord timestamp string.
 * Timestamps are stored as "M/D/YY H:MM:SSa/p"; this returns "M/D/YY".
 */
export function formatDate(val: string | null | undefined): string {
  return val ? val.split(" ")[0] : "";
}

/**
 * Format an hour-bucket window start ("YYYY-MM-DD HH:00") as a 2-hour clock
 * window, e.g. "2024-01-02 08:00–10:00". The metric buckets hourly, so the
 * window spans the start hour through start+2h (wrapping past midnight).
 */
export function formatTwoHourWindow(start: string | null | undefined): string {
  if (!start) return "";
  const [date, time] = start.split(" ");
  if (!time) return start;
  const startHour = parseInt(time.split(":")[0], 10);
  if (Number.isNaN(startHour)) return start;
  const endHour = (startHour + 2) % 24;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date} ${pad(startHour)}:00–${pad(endHour)}:00`;
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
