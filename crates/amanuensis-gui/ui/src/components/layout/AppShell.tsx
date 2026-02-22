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
import { LogSearchView } from "../views/LogSearchView";
import type { ViewType } from "../../types";

const TABS: { id: ViewType; label: string; visibleFor?: string[] }[] = [
  { id: "summary", label: "Summary" },
  { id: "kills", label: "Kills" },
  { id: "trainers", label: "Trainers" },
  { id: "rank-modifiers", label: "Rank Modifiers" },
  { id: "coins", label: "Coins" },
  { id: "pets", label: "Pets", visibleFor: ["Healer"] },
  { id: "lastys", label: "Lastys", visibleFor: ["Ranger"] },
  { id: "equipment", label: "Equipment" },
  { id: "fighter-stats", label: "Stats" },
  { id: "log-search", label: "Log Search" },
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
    case "log-search":
      return <LogSearchView />;
  }
}

export function AppShell() {
  const { activeView, setActiveView, selectedCharacterId, characters, dbPath } =
    useStore();

  const selectedCharacter = characters.find(
    (c) => c.id === selectedCharacterId,
  );

  // Fall back to summary if current view is hidden for this character's profession
  const activeTab = TABS.find((t) => t.id === activeView);
  const effectiveView =
    selectedCharacter &&
    activeTab?.visibleFor &&
    !activeTab.visibleFor.includes(selectedCharacter.profession)
      ? "summary"
      : activeView;

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
                  !tab.visibleFor ||
                  tab.visibleFor.includes(selectedCharacter.profession),
              ).map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveView(tab.id)}
                  className={`px-4 py-2.5 text-sm font-medium transition-colors ${
                    effectiveView === tab.id
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
              <ViewContent view={effectiveView} />
            </div>
          </>
        ) : (
          <div className="flex flex-1 items-center justify-center text-[var(--color-text-muted)]">
            <div className="max-w-md text-center">
              <div className="text-4xl">Amanuensis</div>
              {!dbPath ? (
                <div className="mt-6 rounded-lg border border-[var(--color-border)] bg-[var(--color-card)] p-5 text-left">
                  <div className="mb-2 text-sm font-semibold text-[var(--color-text)]">
                    Getting Started
                  </div>
                  <p className="text-sm leading-relaxed">
                    Click <span className="font-medium text-[var(--color-accent)]">Scan Log Folder(s)</span> in
                    the sidebar and select your Clan Lord log folder. A database will be created automatically.
                  </p>
                  <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-muted)]">
                    Characters are detected automatically from log content, so mixed-character
                    folders work fine. Check <span className="italic">Deep scan</span> if your logs
                    are nested inside multiple folders.
                  </p>
                </div>
              ) : (
                <div className="mt-2 text-sm">
                  Select a character to get started
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
