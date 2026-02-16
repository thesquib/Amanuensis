const PROFESSION_COLORS: Record<string, string> = {
  Fighter: "bg-red-600",
  Healer: "bg-green-600",
  Mystic: "bg-purple-600",
  Ranger: "bg-amber-600",
  Bloodmage: "bg-rose-800",
  Champion: "bg-blue-600",
  Unknown: "bg-gray-600",
};

interface ProfessionBadgeProps {
  profession: string;
}

export function ProfessionBadge({ profession }: ProfessionBadgeProps) {
  const color = PROFESSION_COLORS[profession] ?? "bg-gray-600";
  return (
    <span
      className={`inline-block rounded px-2 py-0.5 text-xs font-medium text-white ${color}`}
    >
      {profession}
    </span>
  );
}
