/**
 * Returns a human-readable relative time string for a date, e.g.:
 *   "today", "3 days ago", "2 months ago", "1 year, 4 months ago"
 */
export function timeAgo(dateStr: string | null | undefined): string | undefined {
  if (!dateStr) return undefined;
  // SQLite stores dates as "YYYY-MM-DD HH:MM:SS"; replace space with T for reliable parsing
  const date = new Date(dateStr.replace(" ", "T"));
  if (isNaN(date.getTime())) return undefined;

  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  if (diffMs < 0) return undefined;

  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));
  if (diffDays === 0) return "today";
  if (diffDays === 1) return "1 day ago";
  if (diffDays < 30) return `${diffDays} days ago`;

  // Use calendar months for accuracy
  const totalMonths =
    (now.getFullYear() - date.getFullYear()) * 12 +
    (now.getMonth() - date.getMonth());
  const years = Math.floor(totalMonths / 12);
  const months = totalMonths % 12;

  if (years === 0) return `${totalMonths} month${totalMonths !== 1 ? "s" : ""} ago`;
  if (months === 0) return `${years} year${years !== 1 ? "s" : ""} ago`;
  return `${years} year${years !== 1 ? "s" : ""}, ${months} month${months !== 1 ? "s" : ""} ago`;
}
