import { useState, useEffect, useRef } from "react";
import {
  getCharacterPortraitPath,
  fetchCharacterPortrait,
} from "../../lib/commands";

interface CharacterPortraitProps {
  name: string;
  className?: string;
}

export function CharacterPortrait({ name, className }: CharacterPortraitProps) {
  const [src, setSrc] = useState<string | null>(null);
  const fetchedRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      // Show cached version immediately if available
      const cached = await getCharacterPortraitPath(name);
      if (!cancelled && cached) {
        setSrc(cached);
      }

      // Always fetch fresh from server once per character load
      if (fetchedRef.current === name) return;
      fetchedRef.current = name;

      const fresh = await fetchCharacterPortrait(name);
      if (!cancelled && fresh) {
        setSrc(fresh);
      }
    }

    setSrc(null);
    fetchedRef.current = null;
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
      className={className ?? "h-16 w-auto rounded-lg"}
      style={{ imageRendering: "auto" }}
    />
  );
}
