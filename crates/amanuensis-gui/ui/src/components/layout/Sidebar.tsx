import { open, save, confirm, message } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useCallback } from "react";
import { useStore } from "../../lib/store";
import {
  openDatabase,
  listCharacters,
  getScannedLogCount,
  getLogLineCount,
  getDefaultDbPath,
  scanLogs,
  scanFiles,
  getKills,
  getTrainers,
  getPets,
  getLastys,
  resetDatabase,
  importScribiusDb,
} from "../../lib/commands";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { ProfessionBadge } from "../shared/ProfessionBadge";
import { ProgressBar } from "../shared/ProgressBar";
import type { ScanProgress } from "../../types";
import type { Theme } from "../../lib/store";

export function Sidebar() {
  const {
    dbPath,
    setDbPath,
    logFolder,
    setLogFolder,
    characters,
    setCharacters,
    selectedCharacterId,
    selectCharacter,
    isScanning,
    setIsScanning,
    scanProgress,
    setScanProgress,
    scannedLogCount,
    setScannedLogCount,
    setKills,
    setTrainers,
    setPets,
    setLastys,
    recursiveScan,
    setRecursiveScan,
    excludeLowCL,
    setExcludeLowCL,
    excludeUnknown,
    setExcludeUnknown,
    indexLogLines,
    setIndexLogLines,
    setLogLineCount,
    theme,
    setTheme,
  } = useStore();

  // Listen for scan progress events
  useEffect(() => {
    const unlisten = listen<ScanProgress>("scan-progress", (event) => {
      setScanProgress(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setScanProgress]);

  const loadCharacterData = useCallback(
    async (charId: number) => {
      const [kills, trainers, pets, lastys] = await Promise.all([
        getKills(charId),
        getTrainers(charId),
        getPets(charId),
        getLastys(charId),
      ]);
      setKills(kills);
      setTrainers(trainers);
      setPets(pets);
      setLastys(lastys);
    },
    [setKills, setTrainers, setPets, setLastys],
  );

  const handleSelectCharacter = useCallback(
    async (charId: number) => {
      selectCharacter(charId);
      await loadCharacterData(charId);
    },
    [selectCharacter, loadCharacterData],
  );

  const loadDatabase = useCallback(
    async (path: string) => {
      await openDatabase(path);
      setDbPath(path);
      localStorage.setItem("amanuensis_last_db", path);
      const chars = await listCharacters();
      setCharacters(chars);
      const count = await getScannedLogCount();
      setScannedLogCount(count);
      const lineCount = await getLogLineCount();
      setLogLineCount(lineCount);
      if (chars.length > 0 && chars[0].id !== null) {
        await handleSelectCharacter(chars[0].id);
      }
    },
    [setDbPath, setCharacters, setScannedLogCount, setLogLineCount, handleSelectCharacter],
  );

  // Auto-open database on startup: last-used path, or default app data dir
  useEffect(() => {
    const lastDb = localStorage.getItem("amanuensis_last_db");
    if (lastDb) {
      loadDatabase(lastDb).catch(() => {
        // DB file may have been deleted — clear stale entry and fall back to default
        localStorage.removeItem("amanuensis_last_db");
        getDefaultDbPath().then((p) => loadDatabase(p)).catch(console.error);
      });
    } else {
      getDefaultDbPath().then((p) => loadDatabase(p)).catch(console.error);
    }
  }, [loadDatabase]);

  const handleOpenDb = useCallback(async () => {
    const selected = await open({
      filters: [{ name: "SQLite Database", extensions: ["db", "sqlite"] }],
    });
    if (selected) {
      const path = typeof selected === "string" ? selected : selected;
      try {
        await loadDatabase(path);
      } catch (e) {
        console.error("Failed to open database:", e);
      }
    }
  }, [loadDatabase]);

  const ensureDb = useCallback(async (): Promise<boolean> => {
    if (dbPath) return true;
    const defaultPath = await getDefaultDbPath();
    await openDatabase(defaultPath);
    setDbPath(defaultPath);
    localStorage.setItem("amanuensis_last_db", defaultPath);
    return true;
  }, [dbPath, setDbPath]);

  const finishScan = useCallback(async () => {
    const chars = await listCharacters();
    setCharacters(chars);
    const count = await getScannedLogCount();
    setScannedLogCount(count);
    const lineCount = await getLogLineCount();
    setLogLineCount(lineCount);
    if (chars.length > 0 && chars[0].id !== null) {
      await handleSelectCharacter(chars[0].id);
    }
  }, [setCharacters, setScannedLogCount, setLogLineCount, handleSelectCharacter]);

  const handleScanFolder = useCallback(async () => {
    if (!(await ensureDb())) return;

    const folder = await open({ directory: true, recursive: true, title: "Select Log Folder" });
    if (!folder) return;
    const folderPath = typeof folder === "string" ? folder : folder;

    setLogFolder(folderPath);
    setIsScanning(true);
    setScanProgress(null);

    try {
      await scanLogs(folderPath, false, recursiveScan, indexLogLines);
      await finishScan();
    } catch (e) {
      console.error("Scan failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [ensureDb, setLogFolder, setIsScanning, setScanProgress, finishScan, recursiveScan, indexLogLines]);

  const handleScanFiles = useCallback(async () => {
    if (!(await ensureDb())) return;

    const selected = await open({
      multiple: true,
      filters: [{ name: "Log Files", extensions: ["txt"] }],
      title: "Select Log Files",
    });
    if (!selected) return;
    const files = Array.isArray(selected) ? selected : [selected];

    setIsScanning(true);
    setScanProgress(null);

    try {
      await scanFiles(files, false, indexLogLines);
      await finishScan();
    } catch (e) {
      console.error("Scan failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [ensureDb, setIsScanning, setScanProgress, finishScan, indexLogLines]);

  const handleReset = useCallback(async () => {
    if (!dbPath) return;
    const confirmed = await confirm(
      "This will permanently delete ALL scanned data including characters, kills, trainers, pets, and lastys. You will need to rescan your logs to restore this data.\n\nAre you sure?",
      { title: "Reset Database", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      await resetDatabase();
      setCharacters([]);
      selectCharacter(null as unknown as number);
      setKills([]);
      setTrainers([]);
      setPets([]);
      setLastys([]);
      setScannedLogCount(0);
      setLogLineCount(0);
    } catch (e) {
      console.error("Reset failed:", e);
    }
  }, [dbPath, setCharacters, selectCharacter, setKills, setTrainers, setPets, setLastys, setScannedLogCount, setLogLineCount]);

  const handleImportScribius = useCallback(async () => {
    // Pick the Scribius .db file
    const scribiusFile = await open({
      filters: [{ name: "Scribius Database", extensions: ["db", "sqlite"] }],
      title: "Select Scribius Database",
    });
    if (!scribiusFile) return;

    // Pick where to save the new Amanuensis database
    const outputFile = await save({
      title: "Save Amanuensis Database As",
      filters: [{ name: "SQLite Database", extensions: ["db"] }],
      defaultPath: "amanuensis.db",
    });
    if (!outputFile) return;

    try {
      const result = await importScribiusDb(scribiusFile, outputFile);
      await loadDatabase(outputFile);

      const parts = [`Imported ${result.characters_imported} character(s)`];
      if (result.trainers_imported > 0) parts.push(`${result.trainers_imported} trainers`);
      if (result.kills_imported > 0) parts.push(`${result.kills_imported} kills`);
      if (result.pets_imported > 0) parts.push(`${result.pets_imported} pets`);
      if (result.lastys_imported > 0) parts.push(`${result.lastys_imported} lastys`);
      if (result.characters_skipped > 0) parts.push(`${result.characters_skipped} skipped`);

      await message(parts.join(", ") + ".", { title: "Import Complete" });

      if (result.warnings.length > 0) {
        await message(result.warnings.join("\n"), { title: "Import Warnings", kind: "warning" });
      }
    } catch (e) {
      await message(String(e), { title: "Import Failed", kind: "error" });
    }
  }, [loadDatabase]);

  return (
    <div className="flex h-full w-60 flex-col border-r border-[var(--color-border)] bg-[var(--color-sidebar)]">
      {/* Header */}
      <div className="border-b border-[var(--color-border)] p-3">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-bold tracking-wide">Amanuensis</h1>
          <select
            value={theme}
            onChange={(e) => setTheme(e.target.value as Theme)}
            className="rounded border border-[var(--color-border)] bg-[var(--color-card)] px-1.5 py-0.5 text-xs text-[var(--color-text)] outline-none"
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
            <option value="midnight">Midnight</option>
          </select>
        </div>
        <div className="mt-1 text-xs text-[var(--color-text-muted)]">
          Clan Lord Log Analyzer
        </div>
      </div>

      {/* Actions */}
      <div className="flex flex-col gap-2 border-b border-[var(--color-border)] p-3">
        <button
          onClick={handleOpenDb}
          disabled={isScanning}
          className="rounded bg-[var(--color-card)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--color-card)]/80 disabled:opacity-50"
        >
          Open Database
        </button>
        <button
          onClick={handleScanFolder}
          disabled={isScanning}
          className="rounded bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80 disabled:opacity-50"
        >
          {isScanning ? "Scanning..." : "Scan Log Folder"}
        </button>
        <button
          onClick={handleScanFiles}
          disabled={isScanning}
          className="rounded bg-[var(--color-accent)]/80 px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/60 disabled:opacity-50"
        >
          Scan Log Files
        </button>
        <button
          onClick={handleImportScribius}
          disabled={isScanning}
          className="rounded bg-[var(--color-card)] px-3 py-1.5 text-sm font-medium hover:bg-[var(--color-card)]/80 disabled:opacity-50"
        >
          Import Scribius DB
        </button>
        <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
          <input
            type="checkbox"
            checked={recursiveScan}
            onChange={(e) => setRecursiveScan(e.target.checked)}
            disabled={isScanning}
            className="accent-[var(--color-accent)]"
          />
          Deep scan (find logs recursively)
        </label>
        <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
          <input
            type="checkbox"
            checked={indexLogLines}
            onChange={(e) => setIndexLogLines(e.target.checked)}
            disabled={isScanning}
            className="accent-[var(--color-accent)]"
          />
          Index logs for search
        </label>
        <div className="text-[10px] text-[var(--color-text-muted)]/60 leading-tight">
          Characters are detected automatically from log content. Mixed-character folders are supported.
        </div>
      </div>

      {/* Scan progress */}
      {isScanning && scanProgress && (
        <div className="border-b border-[var(--color-border)] p-3">
          <ProgressBar
            current={scanProgress.current_file}
            total={scanProgress.total_files}
            label={scanProgress.filename}
          />
        </div>
      )}

      {/* Info */}
      {dbPath && (
        <div className="border-b border-[var(--color-border)] px-3 py-2 text-xs text-[var(--color-text-muted)]">
          <div className="flex items-center gap-1">
            <div className="min-w-0 flex-1 truncate" title={dbPath}>
              DB: {dbPath.split("/").pop()}
            </div>
            <button
              onClick={() => {
                const dir = dbPath.substring(0, dbPath.lastIndexOf("/"));
                shellOpen(dir);
              }}
              title="Show in Finder"
              className="shrink-0 rounded px-1 py-0.5 text-[10px] text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]"
            >
              Reveal
            </button>
          </div>
          {logFolder && (
            <div className="mt-1 truncate" title={logFolder}>
              Logs: {logFolder.split("/").pop()}
            </div>
          )}
          <div className="mt-1">{scannedLogCount} files scanned</div>
          <button
            onClick={handleReset}
            disabled={isScanning}
            className="mt-2 w-full rounded px-2 py-1 text-xs disabled:opacity-50"
            style={{
              backgroundColor: "var(--color-danger-bg)",
              color: "var(--color-danger)",
            }}
            onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "var(--color-danger-bg-hover)")}
            onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "var(--color-danger-bg)")}

          >
            Reset Database
          </button>
        </div>
      )}

      {/* Character list filters */}
      {characters.length > 0 && (
        <div className="flex flex-col gap-1 border-b border-[var(--color-border)] px-3 py-2">
          <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
            <input
              type="checkbox"
              checked={excludeLowCL}
              onChange={(e) => setExcludeLowCL(e.target.checked)}
              className="accent-[var(--color-accent)]"
            />
            Exclude Lvl &lt; 1
          </label>
          <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
            <input
              type="checkbox"
              checked={excludeUnknown}
              onChange={(e) => setExcludeUnknown(e.target.checked)}
              className="accent-[var(--color-accent)]"
            />
            Exclude Unknown
          </label>
        </div>
      )}

      {/* Character list */}
      <div className="min-h-0 flex-1 overflow-y-auto">
        {characters
          .filter((char) => {
            if (excludeLowCL && char.coin_level < 1) return false;
            if (excludeUnknown && char.profession === "Unknown") return false;
            return true;
          })
          .map((char) => (
          <button
            key={char.id}
            onClick={() => char.id !== null && handleSelectCharacter(char.id)}
            className={`flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[var(--color-card)]/30 ${
              selectedCharacterId === char.id
                ? "bg-[var(--color-card)]/50"
                : ""
            }`}
          >
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{char.name}</div>
              <div className="flex items-center gap-2">
                <ProfessionBadge profession={char.profession} />
                <span className="text-xs text-[var(--color-text-muted)]">
                  Lvl {char.coin_level}
                </span>
              </div>
            </div>
          </button>
        ))}
        {characters.length === 0 && dbPath && (
          <div className="p-3 text-center text-xs text-[var(--color-text-muted)]">
            No characters found.
            <br />
            Scan logs to get started.
          </div>
        )}
        {!dbPath && (
          <div className="p-3 text-center text-xs text-[var(--color-text-muted)]">
            Scan logs to get started.
          </div>
        )}
      </div>
    </div>
  );
}
