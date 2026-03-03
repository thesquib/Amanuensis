// ---------------------------------------------------------------------------
// localStorage keys — single source of truth
// ---------------------------------------------------------------------------
export const STORAGE_KEYS = {
  LAST_DB: "amanuensis_last_db",
  LAST_CHARACTER: "amanuensis_last_character",
  THEME: "amanuensis_theme",
  INDEX_LOGS: "amanuensis_index_logs",
  COLLAPSED_TRAINERS: "amanuensis_collapsed_trainers",
  COLLAPSED_RANK_MODIFIERS: "amanuensis_collapsed_rankModifiers",
} as const;

// ---------------------------------------------------------------------------
// Trainer profession display order
// ---------------------------------------------------------------------------
export const PROFESSION_ORDER = [
  "Fighter",
  "Healer",
  "Mystic",
  "Ranger",
  "Bloodmage",
  "Champion",
  "Language",
  "Arts",
  "Trades",
  "Other",
] as const;

export type Profession = (typeof PROFESSION_ORDER)[number];
