import { create } from "zustand";
import type {
  Character,
  Kill,
  Trainer,
  Pet,
  Lasty,
  ScanProgress,
  ViewType,
} from "../types";

export type Theme = "dark" | "light" | "midnight";

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

  // Character data cache
  kills: Kill[];
  setKills: (kills: Kill[]) => void;
  trainers: Trainer[];
  setTrainers: (trainers: Trainer[]) => void;
  pets: Pet[];
  setPets: (pets: Pet[]) => void;
  lastys: Lasty[];
  setLastys: (lastys: Lasty[]) => void;

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

  // Theme
  theme: Theme;
  setTheme: (theme: Theme) => void;

  // Loading
  isLoading: boolean;
  setIsLoading: (loading: boolean) => void;
}

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

  kills: [],
  setKills: (kills) => set({ kills }),
  trainers: [],
  setTrainers: (trainers) => set({ trainers }),
  pets: [],
  setPets: (pets) => set({ pets }),
  lastys: [],
  setLastys: (lastys) => set({ lastys }),

  isScanning: false,
  setIsScanning: (scanning) => set({ isScanning: scanning }),
  scanProgress: null,
  setScanProgress: (progress) => set({ scanProgress: progress }),

  scannedLogCount: 0,
  setScannedLogCount: (count) => set({ scannedLogCount: count }),

  recursiveScan: false,
  setRecursiveScan: (recursive) => set({ recursiveScan: recursive }),

  excludeLowCL: true,
  setExcludeLowCL: (exclude) => set({ excludeLowCL: exclude }),
  excludeUnknown: true,
  setExcludeUnknown: (exclude) => set({ excludeUnknown: exclude }),

  theme: (localStorage.getItem("amanuensis_theme") as Theme) || "dark",
  setTheme: (theme) => {
    if (theme === "dark") {
      delete document.documentElement.dataset.theme;
    } else {
      document.documentElement.dataset.theme = theme;
    }
    localStorage.setItem("amanuensis_theme", theme);
    set({ theme });
  },

  isLoading: false,
  setIsLoading: (loading) => set({ isLoading: loading }),
}));
