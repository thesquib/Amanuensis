import { useStore } from "../../lib/store";
import { Sidebar } from "./Sidebar";
import { SummaryView } from "../views/SummaryView";
import { KillsView } from "../views/KillsView";
import { TrainersView } from "../views/TrainersView";
import { CoinsView } from "../views/CoinsView";
import { PetsView } from "../views/PetsView";
import { LastysView } from "../views/LastysView";
import { EquipmentView } from "../views/EquipmentView";
import { RankModifiersView } from "../views/RankModifiersView";
import { FighterStatsView } from "../views/FighterStatsView";
import type { ViewType } from "../../types";

const TABS: { id: ViewType; label: string; fighterOnly?: boolean }[] = [
  { id: "summary", label: "Summary" },
  { id: "kills", label: "Kills" },
  { id: "trainers", label: "Trainers" },
  { id: "rank-modifiers", label: "Rank Modifiers" },
  { id: "coins", label: "Coins" },
  { id: "pets", label: "Pets" },
  { id: "lastys", label: "Lastys" },
  { id: "equipment", label: "Equipment" },
  { id: "fighter-stats", label: "Fighter Stats", fighterOnly: true },
];

function ViewContent({ view }: { view: ViewType }) {
  switch (view) {
    case "summary":
      return <SummaryView />;
    case "kills":
      return <KillsView />;
    case "trainers":
      return <TrainersView />;
    case "rank-modifiers":
      return <RankModifiersView />;
    case "coins":
      return <CoinsView />;
    case "pets":
      return <PetsView />;
    case "lastys":
      return <LastysView />;
    case "equipment":
      return <EquipmentView />;
    case "fighter-stats":
      return <FighterStatsView />;
  }
}

export function AppShell() {
  const { activeView, setActiveView, selectedCharacterId, characters } =
    useStore();

  const selectedCharacter = characters.find(
    (c) => c.id === selectedCharacterId,
  );

  return (
    <div className="flex h-screen">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        {selectedCharacter ? (
          <>
            {/* Tab bar */}
            <div className="flex border-b border-[var(--color-border)] bg-[var(--color-sidebar)]">
              {TABS.filter(
                (tab) =>
                  !tab.fighterOnly ||
                  ["Fighter", "Ranger", "Champion", "Bloodmage"].includes(
                    selectedCharacter.profession,
                  ),
              ).map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveView(tab.id)}
                  className={`px-4 py-2.5 text-sm font-medium transition-colors ${
                    activeView === tab.id
                      ? "border-b-2 border-[var(--color-accent)] text-[var(--color-accent)]"
                      : "text-[var(--color-text-muted)] hover:text-[var(--color-text)]"
                  }`}
                >
                  {tab.label}
                </button>
              ))}
              <div className="flex-1" />
              <div className="flex items-center px-4 text-sm text-[var(--color-text-muted)]">
                {selectedCharacter.name}
              </div>
            </div>
            {/* View content */}
            <div className="min-h-0 flex-1 overflow-auto p-4">
              <ViewContent view={activeView} />
            </div>
          </>
        ) : (
          <div className="flex flex-1 items-center justify-center text-[var(--color-text-muted)]">
            <div className="text-center">
              <div className="text-4xl">Scribius</div>
              <div className="mt-2 text-sm">
                Select a character or scan logs to get started
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
