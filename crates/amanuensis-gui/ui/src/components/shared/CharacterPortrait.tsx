import { useState, useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  getCharacterPortraitPath,
  fetchCharacterPortrait,
} from "../../lib/commands";

interface CharacterPortraitProps {
  name: string;
}

export function CharacterPortrait({ name }: CharacterPortraitProps) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      // Check cache first
      let path = await getCharacterPortraitPath(name);
      if (!path) {
        // Try fetching from Rank Tracker
        path = await fetchCharacterPortrait(name);
      }
      if (!cancelled && path) {
        setSrc(convertFileSrc(path));
      }
    }

    setSrc(null);
    load().catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [name]);

  if (!src) return null;

  return (
    <img
      src={src}
      alt={`${name} portrait`}
      className="h-16 w-auto rounded-lg"
      style={{ imageRendering: "auto" }}
    />
  );
}
