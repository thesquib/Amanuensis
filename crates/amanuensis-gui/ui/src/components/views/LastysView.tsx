import { useMemo } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { useStore } from "../../lib/store";
import { DataTable } from "../shared/DataTable";
import { useCreatureContextMenu } from "../shared/useCreatureContextMenu";
import type { Lasty } from "../../types";

const columnHelper = createColumnHelper<Lasty>();

function buildColumns(
  onCreatureContextMenu: (e: React.MouseEvent, name: string) => void,
) {
  return [
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
  columnHelper.accessor("lasty_type", {
    header: "Type",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("message_count", {
    header: "Count",
    cell: (info) => info.getValue(),
  }),
  columnHelper.accessor("kills_left", {
    header: "Remaining",
    cell: (info) => {
      if (info.row.original.finished) return "";
      const v = info.getValue();
      return v != null ? `< ${v}` : "";
    },
  }),
  columnHelper.accessor("finished", {
    header: "Completed",
    cell: (info) => {
      if (!info.getValue()) return "No";
      const type = info.row.original.lasty_type;
      return type || "Yes";
    },
  }),
  columnHelper.accessor("first_seen_date", {
    header: "First Seen",
    cell: (info) => info.getValue() ?? "",
  }),
  columnHelper.accessor("last_seen_date", {
    header: "Last Seen",
    cell: (info) => info.getValue() ?? "",
  }),
  columnHelper.accessor("completed_date", {
    header: "Completed Date",
    cell: (info) => info.getValue() ?? "",
  }),
  columnHelper.accessor("abandoned_date", {
    header: "Abandoned",
    cell: (info) => info.getValue() ?? "",
  }),
  ];
}

export function LastysView() {
  const { lastys } = useStore();
  const creatureMenu = useCreatureContextMenu();
  const columns = useMemo(() => buildColumns(creatureMenu.openFor), [creatureMenu.openFor]);

  return (
    <div>
      <div className="mb-4 text-sm text-[var(--color-text-muted)]">
        {lastys.length} lasty record{lastys.length !== 1 ? "s" : ""}
      </div>
      <DataTable data={lastys} columns={columns} />
      {creatureMenu.element}
    </div>
  );
}
