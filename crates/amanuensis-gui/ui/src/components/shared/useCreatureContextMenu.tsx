import { useCallback, useMemo, useState } from "react";
import { ContextMenu, type ContextMenuItem } from "./ContextMenu";
import { useOpenExternal } from "./useOpenExternal";
import { familyPageUrl, getCreatureFamily } from "../../lib/bestiary";

/**
 * Right-click menu for a creature name, shared by every surface that shows one.
 *
 * The family is derived from the creature name via the bundled bestiary, so a caller only
 * needs the name — no row type has to carry a `family` field.
 *
 * Usage:
 *   const creatureMenu = useCreatureContextMenu();
 *   <span onContextMenu={(e) => creatureMenu.openFor(e, name)}>{name}</span>
 *   ...
 *   {creatureMenu.element}
 *
 * One hook instance per view is enough — the menu is a singleton keyed on whichever
 * creature was last right-clicked.
 */
export function useCreatureContextMenu() {
  const [menu, setMenu] = useState<{ x: number; y: number; name: string } | null>(null);
  const { open: openLink, element: linkFailure } = useOpenExternal();

  const openFor = useCallback((e: React.MouseEvent, name: string) => {
    if (!name) return;
    e.preventDefault();
    // Stop the surrounding row/card handlers (e.g. KillsView opens a modal on click)
    // from also reacting to the right-click.
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, name });
  }, []);

  const close = useCallback(() => setMenu(null), []);

  const items = useMemo<ContextMenuItem[]>(() => {
    if (!menu) return [];
    const family = getCreatureFamily(menu.name);
    const url = familyPageUrl(family);
    return [
      url
        ? {
            label: `Open in Bestiary — ${family}`,
            onSelect: () => openLink(url),
          }
        : {
            // Extinct creatures have no family page, and an unrecognised name has no family.
            label: family ? "No bestiary page for this family" : "Not in the bestiary",
            onSelect: () => {},
            disabled: true,
          },
      {
        label: "Copy creature name",
        onSelect: () => {
          navigator.clipboard.writeText(menu.name).catch(() => {});
        },
      },
    ];
  }, [menu, openLink]);

  // The failure notice outlives the menu — selecting the item closes it.
  const element = (
    <>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={items} onClose={close} />}
      {linkFailure}
    </>
  );

  return { openFor, element };
}
