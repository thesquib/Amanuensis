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
      </div>
    </div>
  );
}
