import { getCreatureImageUrl } from "../../lib/bestiary";

interface CreatureImageProps {
  creatureName: string;
  className?: string;
}

export function CreatureImage({ creatureName, className }: CreatureImageProps) {
  const url = getCreatureImageUrl(creatureName);
  if (!url) return null;

  return (
    <img
      src={url}
      alt={creatureName}
      className={className}
      style={{ imageRendering: "pixelated" }}
    />
  );
}
