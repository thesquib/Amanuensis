import type { Trainer } from "../types";

export function effectiveRanks(t: Trainer): number {
  switch (t.rank_mode) {
    case "override":
      return t.modified_ranks;
    case "override_until_date":
      return t.modified_ranks + t.ranks + t.apply_learning_ranks;
    default:
      return t.ranks + t.modified_ranks + t.apply_learning_ranks;
  }
}
