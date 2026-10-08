import { useEffect, useMemo, useState } from "react";
import { createColumnHelper } from "@tanstack/react-table";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useStore } from "../../lib/store";
import { deletePet, getPets, mergePets } from "../../lib/commands";
import { DataTable } from "../shared/DataTable";
import { useCreatureContextMenu } from "../shared/useCreatureContextMenu";
import type { Pet } from "../../types";

const columnHelper = createColumnHelper<Pet>();

const buttonClass =
  "rounded border border-[var(--color-border)] px-3 py-1 text-sm hover:bg-[var(--color-card)] disabled:opacity-50";

function buildColumns(
  selected: Set<string>,
  toggle: (name: string) => void,
  onCreatureContextMenu: (e: React.MouseEvent, name: string) => void,
) {
  return [
    columnHelper.display({
      id: "select",
      header: "",
      cell: (info) => (
        <input
          type="checkbox"
          aria-label={`Select ${info.row.original.pet_name}`}
          checked={selected.has(info.row.original.pet_name)}
          onChange={() => toggle(info.row.original.pet_name)}
        />
      ),
    }),
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
  const { pets, setPets, selectedCharacterId } = useStore();
  const creatureMenu = useCreatureContextMenu();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [mergeTarget, setMergeTarget] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Selection is per character.
  useEffect(() => {
    setSelected(new Set());
    setError(null);
  }, [selectedCharacterId]);

  const selectedNames = useMemo(
    () => pets.map((p) => p.pet_name).filter((n) => selected.has(n)),
    [pets, selected],
  );

  // Default the merge target to the first selected pet; keep the user's pick while it stays selected.
  useEffect(() => {
    if (!selectedNames.includes(mergeTarget)) setMergeTarget(selectedNames[0] ?? "");
  }, [selectedNames, mergeTarget]);

  const toggle = (name: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });

  const columns = useMemo(
    () => buildColumns(selected, toggle, creatureMenu.openFor),
    [selected, creatureMenu.openFor],
  );

  const run = async (action: (charId: number) => Promise<void>) => {
    if (selectedCharacterId == null) return;
    setBusy(true);
    setError(null);
    try {
      await action(selectedCharacterId);
      setPets(await getPets(selectedCharacterId));
      setSelected(new Set());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async () => {
    const names = selectedNames;
    const ok = await confirm(
      `Delete ${names.length === 1 ? `"${names[0]}"` : `${names.length} pets`}? They stay deleted after a rescan.`,
      { title: "Delete Pets", kind: "warning" },
    );
    if (!ok) return;
    await run(async (charId) => {
      for (const name of names) await deletePet(charId, name);
    });
  };

  const handleMerge = async () => {
    const sources = selectedNames.filter((n) => n !== mergeTarget);
    const ok = await confirm(
      `Merge ${sources.map((s) => `"${s}"`).join(", ")} into "${mergeTarget}"? This stays in place after a rescan.`,
      { title: "Merge Pets", kind: "warning" },
    );
    if (!ok) return;
    await run((charId) => mergePets(charId, sources, mergeTarget));
  };

  return (
    <div>
      <div className="mb-4 flex flex-wrap items-center gap-3 text-sm">
        <span className="text-[var(--color-text-muted)]">
          {pets.length} pet{pets.length !== 1 ? "s" : ""}
        </span>
        {selectedNames.length > 0 && (
          <>
            <button className={buttonClass} disabled={busy} onClick={handleDelete}>
              Delete ({selectedNames.length})
            </button>
            {selectedNames.length > 1 && (
              <span className="flex items-center gap-2">
                <button className={buttonClass} disabled={busy} onClick={handleMerge}>
                  Merge into
                </button>
                <select
                  value={mergeTarget}
                  onChange={(e) => setMergeTarget(e.target.value)}
                  className="rounded border border-[var(--color-border)] bg-[var(--color-card)] px-2 py-1 text-sm"
                >
                  {selectedNames.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </span>
            )}
          </>
        )}
        {error && <span className="text-red-500">{error}</span>}
      </div>
      <DataTable data={pets} columns={columns} />
      {creatureMenu.element}
    </div>
  );
}
