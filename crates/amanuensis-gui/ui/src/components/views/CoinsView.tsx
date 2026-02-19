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
    header: "Source",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("amount", {
    header: "Amount",
    cell: (info) => info.getValue().toLocaleString(),
  }),
];

export function CoinsView() {
  const { characters, selectedCharacterId } = useStore();
  const char = characters.find((c) => c.id === selectedCharacterId);
  if (!char) return null;

  const sources: CoinSource[] = useMemo(
    () =>
      [
        { source: "Picked Up", amount: char.coins_picked_up },
        { source: "Casino Won", amount: char.casino_won },
        { source: "Casino Lost", amount: -char.casino_lost },
        { source: "Furs", amount: char.fur_coins },
        { source: "Blood", amount: char.blood_coins },
        { source: "Mandibles", amount: char.mandible_coins },
        { source: "Bounties", amount: char.bounty_coins },
        { source: "Chest/Studies", amount: char.chest_coins },
        { source: "Esteem", amount: char.esteem },
        { source: "Darkstone", amount: char.darkstone },
      ].filter((s) => s.amount !== 0),
    [char],
  );

  const totalCoins = sources.reduce((s, c) => s + c.amount, 0);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3">
        <StatCard label="Net Coins" value={totalCoins.toLocaleString()} />
        <StatCard
          label="Coins Picked Up"
          value={char.coins_picked_up.toLocaleString()}
        />
        <StatCard
          label="Casino Net"
          value={(char.casino_won - char.casino_lost).toLocaleString()}
          sub={`Won ${char.casino_won.toLocaleString()} / Lost ${char.casino_lost.toLocaleString()}`}
        />
      </div>
      <div className="min-h-0 flex-1">
        <DataTable data={sources} columns={columns} />
      </div>
    </div>
  );
}
