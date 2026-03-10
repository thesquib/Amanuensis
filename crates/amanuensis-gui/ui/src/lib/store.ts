import { create } from "zustand";
import type { SortingState } from "@tanstack/react-table";
import { STORAGE_KEYS } from "./constants";
import type {
  Character,
  Kill,
  Trainer,
  Pet,
  Lasty,
  ScanProgress,
  ViewType,
  ProcessLog,
} from "../types";

export type Theme = "dark" | "light" | "midnight";

interface DataTableViewState {
  sorting: SortingState;
  globalFilter: string;
}

interface TrainersViewState {
  showZero: boolean;
  showEffective: boolean;
  searchQuery: string;
  collapsedGroups: string[];
}

interface RankModifiersViewState {
  searchQuery: string;
  collapsedGroups: string[];
}

interface RangerStatsViewState {
  activePanel: "studies" | "families" | "targets";
  searchQuery: string;
}

interface AppStore {
  // Database
  dbPath: string | null;
  setDbPath: (path: string | null) => void;

  // Log folder
  logFolder: string | null;
  setLogFolder: (folder: string | null) => void;

  // Characters
  characters: Character[];
  setCharacters: (chars: Character[]) => void;
  selectedCharacterId: number | null;
  selectCharacter: (id: number | null) => void;

  // Active view
  activeView: ViewType;
  setActiveView: (view: ViewType) => void;

  // TS-computed coin level per character (stuffable filter applied)
  coinLevelByCharId: Record<number, number>;
  setCoinLevelForChar: (charId: number, value: number) => void;

  // Character data cache
  kills: Kill[];
  setKills: (kills: Kill[]) => void;
  trainers: Trainer[];
  setTrainers: (trainers: Trainer[]) => void;
  pets: Pet[];
  setPets: (pets: Pet[]) => void;
  lastys: Lasty[];
  setLastys: (lastys: Lasty[]) => void;

  // Process logs
  processLogs: ProcessLog[];
  setProcessLogs: (logs: ProcessLog[]) => void;
  warnsDismissed: boolean;
  setWarnsDismissed: (dismissed: boolean) => void;

  // Scanning
  isScanning: boolean;
  setIsScanning: (scanning: boolean) => void;
  scanProgress: ScanProgress | null;
  setScanProgress: (progress: ScanProgress | null) => void;

  // Scanned log count
  scannedLogCount: number;
  setScannedLogCount: (count: number) => void;

  // Recursive scan toggle
  recursiveScan: boolean;
  setRecursiveScan: (recursive: boolean) => void;

  // Character list filters
  excludeLowCL: boolean;
  setExcludeLowCL: (exclude: boolean) => void;
  excludeUnknown: boolean;
  setExcludeUnknown: (exclude: boolean) => void;

  // Index log lines for search
  indexLogLines: boolean;
  setIndexLogLines: (index: boolean) => void;

  // Log line count (FTS5 indexed)
  logLineCount: number;
  setLogLineCount: (count: number) => void;

  // Theme
  theme: Theme;
  setTheme: (theme: Theme) => void;

  // Loading
  isLoading: boolean;
  setIsLoading: (loading: boolean) => void;

  // Per-view persisted state (survives tab switches)
  viewStates: Record<string, DataTableViewState>;
  setViewSorting: (view: string, sorting: SortingState) => void;
  setViewFilter: (view: string, filter: string) => void;

  trainersViewState: TrainersViewState;
  setTrainersViewState: (patch: Partial<TrainersViewState>) => void;

  rankModifiersViewState: RankModifiersViewState;
  setRankModifiersViewState: (patch: Partial<RankModifiersViewState>) => void;

  rangerStatsViewState: RangerStatsViewState;
  setRangerStatsViewState: (patch: Partial<RangerStatsViewState>) => void;
}

function loadCollapsedGroups(key: string): string[] | null {
  try {
    const stored = localStorage.getItem(key);
    if (stored) return JSON.parse(stored);
  } catch { /* ignore */ }
  return null;
}

function saveCollapsedGroups(key: string, groups: string[]) {
  localStorage.setItem(key, JSON.stringify(groups));
}

const TRAINERS_COLLAPSED_KEY = STORAGE_KEYS.COLLAPSED_TRAINERS;
const RANK_MODIFIERS_COLLAPSED_KEY = STORAGE_KEYS.COLLAPSED_RANK_MODIFIERS;

export const useStore = create<AppStore>((set) => ({
  dbPath: null,
  setDbPath: (path) => set({ dbPath: path }),

  logFolder: null,
  setLogFolder: (folder) => set({ logFolder: folder }),

  characters: [],
  setCharacters: (chars) => set({ characters: chars }),
  selectedCharacterId: null,
  selectCharacter: (id) => set({ selectedCharacterId: id }),

  activeView: "summary",
  setActiveView: (view) => set({ activeView: view }),

  coinLevelByCharId: {},
  setCoinLevelForChar: (charId, value) =>
    set((state) => ({ coinLevelByCharId: { ...state.coinLevelByCharId, [charId]: value } })),

  kills: [],
  setKills: (kills) => set({ kills }),
  trainers: [],
  setTrainers: (trainers) => set({ trainers }),
  pets: [],
  setPets: (pets) => set({ pets }),
  lastys: [],
  setLastys: (lastys) => set({ lastys }),

  processLogs: [],
  setProcessLogs: (logs) => set({ processLogs: logs, warnsDismissed: false }),
  warnsDismissed: false,
  setWarnsDismissed: (dismissed) => set({ warnsDismissed: dismissed }),

  isScanning: false,
  setIsScanning: (scanning) => set({ isScanning: scanning }),
  scanProgress: null,
  setScanProgress: (progress) => set({ scanProgress: progress }),

  scannedLogCount: 0,
  setScannedLogCount: (count) => set({ scannedLogCount: count }),

  recursiveScan: true,
  setRecursiveScan: (recursive) => set({ recursiveScan: recursive }),

  excludeLowCL: true,
  setExcludeLowCL: (exclude) => set({ excludeLowCL: exclude }),
  excludeUnknown: true,
  setExcludeUnknown: (exclude) => set({ excludeUnknown: exclude }),

  indexLogLines: localStorage.getItem(STORAGE_KEYS.INDEX_LOGS) !== "false",
  setIndexLogLines: (index) => {
    localStorage.setItem(STORAGE_KEYS.INDEX_LOGS, String(index));
    set({ indexLogLines: index });
  },

  logLineCount: 0,
  setLogLineCount: (count) => set({ logLineCount: count }),

  theme: (localStorage.getItem(STORAGE_KEYS.THEME) as Theme) || "dark",
  setTheme: (theme) => {
    if (theme === "dark") {
      delete document.documentElement.dataset.theme;
    } else {
      document.documentElement.dataset.theme = theme;
    }
    localStorage.setItem(STORAGE_KEYS.THEME, theme);
    set({ theme });
  },

  isLoading: false,
  setIsLoading: (loading) => set({ isLoading: loading }),

  viewStates: {},
  setViewSorting: (view, sorting) =>
    set((state) => ({
      viewStates: {
        ...state.viewStates,
        [view]: { ...state.viewStates[view], sorting, globalFilter: state.viewStates[view]?.globalFilter ?? "" },
      },
    })),
  setViewFilter: (view, globalFilter) =>
    set((state) => ({
      viewStates: {
        ...state.viewStates,
        [view]: { ...state.viewStates[view], globalFilter, sorting: state.viewStates[view]?.sorting ?? [] },
      },
    })),

  trainersViewState: { showZero: false, showEffective: false, searchQuery: "", collapsedGroups: loadCollapsedGroups(TRAINERS_COLLAPSED_KEY) ?? [] },
  setTrainersViewState: (patch) => {
    if (patch.collapsedGroups) {
      saveCollapsedGroups(TRAINERS_COLLAPSED_KEY, patch.collapsedGroups);
    }
    set((state) => ({
      trainersViewState: { ...state.trainersViewState, ...patch },
    }));
  },

  rankModifiersViewState: { searchQuery: "", collapsedGroups: loadCollapsedGroups(RANK_MODIFIERS_COLLAPSED_KEY) ?? [] },
  setRankModifiersViewState: (patch) => {
    if (patch.collapsedGroups) {
      saveCollapsedGroups(RANK_MODIFIERS_COLLAPSED_KEY, patch.collapsedGroups);
    }
    set((state) => ({
      rankModifiersViewState: { ...state.rankModifiersViewState, ...patch },
    }));
  },

  rangerStatsViewState: { activePanel: "studies", searchQuery: "" },
  setRangerStatsViewState: (patch) =>
    set((state) => ({
      rangerStatsViewState: { ...state.rangerStatsViewState, ...patch },
    })),
}));
