import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import type { Pet } from "../../types";

const columnHelper = createColumnHelper<Pet>();

const columns = [
  columnHelper.accessor("pet_name", {
    header: "Pet Name",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("creature_name", {
    header: "Creature",
    cell: (info) => info.getValue(),
  }),
];

export function PetsView() {
  const { pets } = useStore();

  return (
    <div>
      <div className="mb-4 text-sm text-[var(--color-text-muted)]">
        {pets.length} pet{pets.length !== 1 ? "s" : ""}
      </div>
      <DataTable data={pets} columns={columns} />
    </div>
  );
}
