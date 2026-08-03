import { useMemo } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { useCreatureContextMenu } from "../shared/useCreatureContextMenu";
import type { Pet } from "../../types";

const columnHelper = createColumnHelper<Pet>();

function buildColumns(
  onCreatureContextMenu: (e: React.MouseEvent, name: string) => void,
) {
  return [
    columnHelper.accessor("pet_name", {
      header: "Pet Name",
      cell: (info) => info.getValue(),
    }),
    columnHelper.accessor("creature_name", {
      header: "Creature",
      cell: (info) => (
        <span
          className="cursor-context-menu"
          onContextMenu={(e) => onCreatureContextMenu(e, info.getValue())}
        >
          {info.getValue()}
        </span>
      ),
    }),
  ];
}

export function PetsView() {
  const { pets } = useStore();
  const creatureMenu = useCreatureContextMenu();
  const columns = useMemo(() => buildColumns(creatureMenu.openFor), [creatureMenu.openFor]);

  return (
    <div>
      <div className="mb-4 text-sm text-[var(--color-text-muted)]">
        {pets.length} pet{pets.length !== 1 ? "s" : ""}
      </div>
      <DataTable data={pets} columns={columns} />
      {creatureMenu.element}
    </div>
  );
}
