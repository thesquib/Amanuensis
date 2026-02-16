/** Mirrors Rust `Character` struct */
export interface Character {
  id: number | null;
  name: string;
  profession: string;
  logins: number;
  departs: number;
  deaths: number;
  esteem: number;
  armor: string;
  coins_picked_up: number;
  casino_won: number;
  casino_lost: number;
  chest_coins: number;
  bounty_coins: number;
  fur_coins: number;
  mandible_coins: number;
  blood_coins: number;
  bells_used: number;
  bells_broken: number;
  chains_used: number;
  chains_broken: number;
  shieldstones_used: number;
  shieldstones_broken: number;
  ethereal_portals: number;
  darkstone: number;
  purgatory_pendant: number;
  coin_level: number;
  good_karma: number;
  bad_karma: number;
  start_date: string | null;
  fur_worth: number;
  mandible_worth: number;
  blood_worth: number;
  eps_broken: number;
}

/** Mirrors Rust `Kill` struct */
export interface Kill {
  id: number | null;
  character_id: number;
  creature_name: string;
  killed_count: number;
  slaughtered_count: number;
  vanquished_count: number;
  dispatched_count: number;
  assisted_kill_count: number;
  assisted_slaughter_count: number;
  assisted_vanquish_count: number;
  assisted_dispatch_count: number;
  killed_by_count: number;
  date_first: string | null;
  date_last: string | null;
  creature_value: number;
}

/** Mirrors Rust `Trainer` struct */
export interface Trainer {
  id: number | null;
  character_id: number;
  trainer_name: string;
  ranks: number;
  modified_ranks: number;
  date_of_last_rank: string | null;
}

/** Mirrors Rust `Pet` struct */
export interface Pet {
  id: number | null;
  character_id: number;
  pet_name: string;
  creature_name: string;
}

/** Mirrors Rust `Lasty` struct */
export interface Lasty {
  id: number | null;
  character_id: number;
  creature_name: string;
  lasty_type: string;
  finished: boolean;
  message_count: number;
}

/** Mirrors Rust `ScanResult` struct */
export interface ScanResult {
  characters: number;
  files_scanned: number;
  skipped: number;
  lines_parsed: number;
  events_found: number;
  errors: number;
}

/** Mirrors Rust `ScanProgress` struct */
export interface ScanProgress {
  current_file: number;
  total_files: number;
  filename: string;
}

/** Mirrors Rust `TrainerInfo` struct */
export interface TrainerInfo {
  name: string;
  profession: string | null;
}

export type ViewType =
  | "summary"
  | "kills"
  | "trainers"
  | "rank-modifiers"
  | "coins"
  | "pets"
  | "lastys"
  | "equipment";
