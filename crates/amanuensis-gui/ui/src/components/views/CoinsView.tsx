import { useEffect, useMemo, useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { getBlackjackSummary } from "../../lib/commands";
import { DataTable } from "../shared/DataTable";
import { StatCard } from "../shared/StatCard";
import type { BlackjackSummary } from "../../types";

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
  const scanVersion = useStore((s) => s.scanVersion);
  const char = characters.find((c) => c.id === selectedCharacterId);
  const [blackjack, setBlackjack] = useState<BlackjackSummary | null>(null);

  useEffect(() => {
    if (selectedCharacterId == null) return;
    let cancelled = false;
    setBlackjack(null);
    getBlackjackSummary(selectedCharacterId)
      .then((s) => {
        if (!cancelled) setBlackjack(s);
      })
      .catch((err) => console.error("Failed to load blackjack summary:", err));
    return () => {
      cancelled = true;
    };
  }, [selectedCharacterId, scanVersion, characters]);

  const sources: CoinSource[] = useMemo(
    () =>
      char
        ? [
            { source: "Furs I've recovered have been worth", amount: char.fur_worth },
            { source: "Mandibles I've recovered have been worth", amount: char.mandible_worth },
            { source: "Blood I've recovered have been worth", amount: char.blood_worth },
            { source: "Coins I've earned from all furs", amount: char.fur_coins },
            { source: "Coins I've earned from all mandibles", amount: char.mandible_coins },
            { source: "Coins I've earned from all blood", amount: char.blood_coins },
            { source: "Coins I've earned from all bounties", amount: char.bounty_coins },
            { source: "Coins I've won on Casino Slots", amount: char.casino_won },
            { source: "Coins I've lost on Casino Slots", amount: char.casino_lost },
            { source: "Coins I've won at Blackjack", amount: blackjack?.coins_won ?? 0 },
            { source: "Coins I've lost at Blackjack", amount: blackjack?.coins_lost ?? 0 },
            { source: "Coins I've collected from chest", amount: char.chest_coins },
            { source: "Coins picked up", amount: char.coins_picked_up },
            { source: "Esteem", amount: char.esteem },
            { source: "Darkstone", amount: char.darkstone },
          ].filter((s) => s.amount !== 0)
        : [],
    [char, blackjack],
  );

  if (!char) return null;

  const blackjackNet = blackjack ? blackjack.coins_won - blackjack.coins_lost : 0;
  const totalCoins =
    char.fur_coins + char.mandible_coins + char.blood_coins +
    char.bounty_coins + char.chest_coins + char.coins_picked_up +
    char.casino_won - char.casino_lost + blackjackNet + char.esteem + char.darkstone;

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
        {blackjack && (
          <>
            <StatCard
              label="Blackjack Net"
              value={`${blackjackNet.toLocaleString()}c`}
              sub={`Won ${blackjack.coins_won.toLocaleString()}c / Lost ${blackjack.coins_lost.toLocaleString()}c`}
            />
            <StatCard
              label="Blackjack Hands"
              value={blackjack.hands.toLocaleString()}
              sub={`${blackjack.wins} won / ${blackjack.losses} lost / ${blackjack.pushes} pushed${
                blackjack.naturals > 0 ? ` · ${blackjack.naturals} blackjacks` : ""
              }`}
            />
            <StatCard
              label="Blackjack Win Chance"
              value={`${((blackjack.wins / blackjack.hands) * 100).toFixed(1)}%`}
              sub={
                blackjack.bet_count > 0
                  ? `Average bet ${Math.round(blackjack.bet_total / blackjack.bet_count).toLocaleString()}c`
                  : undefined
              }
            />
          </>
        )}
      </div>
      <div className="min-h-0 flex-1">
        <DataTable data={sources} columns={columns} />
      </div>
    </div>
  );
}
