import { useMemo } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { StatCard } from "../shared/StatCard";

interface CoinSource {
  source: string;
  amount: number;
}

const columnHelper = createColumnHelper<CoinSource>();

const columns = [
  columnHelper.accessor("source", {
    header: "Coin Source",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("amount", {
    header: "Amount",
    cell: (info) => `${info.getValue().toLocaleString()}c`,
  }),
];

export function CoinsView() {
  const { characters, selectedCharacterId } = useStore();
  const char = characters.find((c) => c.id === selectedCharacterId);
  if (!char) return null;

  const sources: CoinSource[] = useMemo(
    () =>
      [
        { source: "Furs I've recovered have been worth", amount: char.fur_worth },
        { source: "Mandibles I've recovered have been worth", amount: char.mandible_worth },
        { source: "Blood I've recovered have been worth", amount: char.blood_worth },
        { source: "Coins I've earned from all furs", amount: char.fur_coins },
        { source: "Coins I've earned from all mandibles", amount: char.mandible_coins },
        { source: "Coins I've earned from all blood", amount: char.blood_coins },
        { source: "Coins I've earned from all bounties", amount: char.bounty_coins },
        { source: "Coins I've won on Casino Slots", amount: char.casino_won },
        { source: "Coins I've lost on Casino Slots", amount: char.casino_lost },
        { source: "Coins I've collected from chest", amount: char.chest_coins },
        { source: "Coins picked up", amount: char.coins_picked_up },
        { source: "Esteem", amount: char.esteem },
        { source: "Darkstone", amount: char.darkstone },
      ].filter((s) => s.amount !== 0),
    [char],
  );

  const totalCoins =
    char.fur_coins + char.mandible_coins + char.blood_coins +
    char.bounty_coins + char.chest_coins + char.coins_picked_up +
    char.casino_won - char.casino_lost + char.esteem + char.darkstone;

  return (
    <div className="flex h-full flex-col">
      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3">
        <StatCard label="Net Coins" value={`${totalCoins.toLocaleString()}c`} />
        <StatCard
          label="Total Loot Worth"
          value={`${(char.fur_worth + char.mandible_worth + char.blood_worth).toLocaleString()}c`}
          sub="Furs + Mandibles + Blood (unshared value)"
        />
        <StatCard
          label="Casino Net"
          value={`${(char.casino_won - char.casino_lost).toLocaleString()}c`}
          sub={`Won ${char.casino_won.toLocaleString()}c / Lost ${char.casino_lost.toLocaleString()}c`}
        />
      </div>
      <div className="min-h-0 flex-1">
        <DataTable data={sources} columns={columns} />
      </div>
    </div>
  );
}
