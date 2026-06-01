import { useState } from "react";
import { getCreatureImageUrl } from "../../lib/bestiary";

interface CreatureImageProps {
  creatureName: string;
  className?: string;
}

export function CreatureImage({ creatureName, className }: CreatureImageProps) {
  const url = getCreatureImageUrl(creatureName);
  const [failed, setFailed] = useState(false);
  // No sprite, or the sprite 404'd: render nothing rather than a broken-image glyph.
  if (!url || failed) return null;

  return (
    <img
      src={url}
      alt={creatureName}
      className={className}
      style={{ imageRendering: "pixelated" }}
      onError={() => setFailed(true)}
    />
  );
}
