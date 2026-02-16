import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import type { Lasty } from "../../types";

const columnHelper = createColumnHelper<Lasty>();

const columns = [
  columnHelper.accessor("creature_name", {
    header: "Creature",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("lasty_type", {
    header: "Type",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("message_count", {
    header: "Count",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("finished", {
    header: "Completed",
    cell: (info) => (info.getValue() ? "Yes" : "No"),
  }),
];

export function LastysView() {
  const { lastys } = useStore();

  return (
    <div>
      <div className="mb-4 text-sm text-[var(--color-text-muted)]">
        {lastys.length} lasty record{lastys.length !== 1 ? "s" : ""}
      </div>
      <DataTable data={lastys} columns={columns} />
    </div>
  );
}
