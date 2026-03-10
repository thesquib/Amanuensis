import { useStore } from "../../lib/store";

interface ProfessionColors {
  vivid: string;
  pastelDark: string;
  pastelLight: string;
}

const PROFESSION_COLORS: Record<string, ProfessionColors> = {
  Fighter:   { vivid: "bg-red-600 text-white",    pastelDark: "bg-red-500/20 text-red-400",      pastelLight: "bg-red-100 text-red-700" },
  Healer:    { vivid: "bg-green-600 text-white",  pastelDark: "bg-green-500/20 text-green-400",  pastelLight: "bg-green-100 text-green-700" },
  Mystic:    { vivid: "bg-purple-600 text-white", pastelDark: "bg-purple-500/20 text-purple-400",pastelLight: "bg-purple-100 text-purple-700" },
  Ranger:    { vivid: "bg-amber-600 text-white",  pastelDark: "bg-amber-500/20 text-amber-400",  pastelLight: "bg-amber-100 text-amber-700" },
  Bloodmage: { vivid: "bg-rose-800 text-white",   pastelDark: "bg-rose-800/30 text-rose-400",    pastelLight: "bg-rose-100 text-rose-700" },
  Champion:  { vivid: "bg-blue-600 text-white",   pastelDark: "bg-blue-500/20 text-blue-400",    pastelLight: "bg-blue-100 text-blue-700" },
  Language:  { vivid: "bg-cyan-600 text-white",   pastelDark: "bg-cyan-500/20 text-cyan-400",    pastelLight: "bg-cyan-100 text-cyan-700" },
  Arts:      { vivid: "bg-pink-600 text-white",   pastelDark: "bg-pink-500/20 text-pink-400",    pastelLight: "bg-pink-100 text-pink-700" },
  Trades:    { vivid: "bg-orange-600 text-white", pastelDark: "bg-orange-500/20 text-orange-400",pastelLight: "bg-orange-100 text-orange-700" },
  Other:     { vivid: "bg-gray-600 text-white",   pastelDark: "bg-gray-500/20 text-gray-400",    pastelLight: "bg-gray-200 text-gray-600" },
  Unknown:   { vivid: "bg-gray-600 text-white",   pastelDark: "bg-gray-500/20 text-gray-400",    pastelLight: "bg-gray-200 text-gray-600" },
};

const FALLBACK: ProfessionColors = { vivid: "bg-gray-600 text-white", pastelDark: "bg-gray-500/20 text-gray-400", pastelLight: "bg-gray-200 text-gray-600" };

interface ProfessionBadgeProps {
  profession: string;
}

export function ProfessionBadge({ profession }: ProfessionBadgeProps) {
  const { theme } = useStore();
  const colors = PROFESSION_COLORS[profession] ?? FALLBACK;

  let colorClass: string;
  if (theme.endsWith("-v2")) {
    colorClass = theme === "light-v2" ? colors.pastelLight : colors.pastelDark;
  } else {
    colorClass = colors.vivid;
  }

  return (
    <span className={`inline-block rounded px-2 py-0.5 text-xs font-medium ${colorClass}`}>
      {profession}
    </span>
  );
}
