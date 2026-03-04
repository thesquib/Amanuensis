import { useStore } from "../../lib/store";
import { StatCard } from "../shared/StatCard";

export function EquipmentView() {
  const { characters, selectedCharacterId } = useStore();
  const char = characters.find((c) => c.id === selectedCharacterId);
  if (!char) return null;

  return (
    <div>
      <h3 className="mb-4 text-lg font-semibold">Equipment Usage</h3>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
        <StatCard
          label="Bells Used"
          value={char.bells_used.toLocaleString()}
          sub={`${char.bells_broken} broken`}
        />
        <StatCard
          label="Chains Used"
          value={char.chains_used.toLocaleString()}
          sub={`${char.chains_broken} broken`}
        />
        <StatCard
          label="Shieldstones Used"
          value={char.shieldstones_used.toLocaleString()}
          sub={`${char.shieldstones_broken} broken`}
        />
        <StatCard
          label="Ethereal Portals"
          value={char.ethereal_portals.toLocaleString()}
        />
        <StatCard
          label="Purgatory Pendant"
          value={char.purgatory_pendant.toLocaleString()}
        />
        <StatCard
          label="Ore Found"
          value={char.ore_found.toLocaleString()}
          sub={char.ore_found > 0
            ? [
                char.iron_ore_found > 0 ? `${char.iron_ore_found} iron` : null,
                char.copper_ore_found > 0 ? `${char.copper_ore_found} copper` : null,
                char.tin_ore_found > 0 ? `${char.tin_ore_found} tin` : null,
                char.gold_ore_found > 0 ? `${char.gold_ore_found} gold` : null,
              ].filter(Boolean).join(" · ") || undefined
            : undefined}
        />
        <StatCard
          label="Wood Taken"
          value={char.wood_taken.toLocaleString()}
          sub={char.wood_taken + char.wood_useless > 0
            ? `${char.wood_useless} useless · ${Math.round(char.wood_taken / (char.wood_taken + char.wood_useless) * 100)}% success`
            : undefined}
        />
      </div>
    </div>
  );
}
