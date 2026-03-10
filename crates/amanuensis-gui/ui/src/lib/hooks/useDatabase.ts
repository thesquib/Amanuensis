import { open, save, message, confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect } from "react";
import { useStore } from "../store";
import { STORAGE_KEYS } from "../constants";
import {
  openDatabase,
  listCharacters,
  getScannedLogCount,
  getLogLineCount,
  getDefaultDbPath,
  getKills,
  getTrainers,
  getPets,
  getLastys,
  resetDatabase,
  deleteAllData,
  importScribiusDb,
  getProcessLogs,
} from "../commands";
import { computeKillStats } from "../killStats";

export function useDatabase() {
  const {
    dbPath,
    setDbPath,
    setCharacters,
    selectedCharacterId,
    selectCharacter,
    isScanning,
    setScannedLogCount,
    setKills,
    setTrainers,
    setPets,
    setLastys,
    setLogLineCount,
    setProcessLogs,
    setCoinLevelForChar,
  } = useStore();

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
      const stats = computeKillStats(kills);
      setCoinLevelForChar(charId, stats.coinLevelKill?.creature_value ?? 0);
    },
    [setKills, setTrainers, setPets, setLastys, setCoinLevelForChar],
  );

  const handleSelectCharacter = useCallback(
    async (charId: number) => {
      selectCharacter(charId);
      localStorage.setItem(STORAGE_KEYS.LAST_CHARACTER, String(charId));
      await loadCharacterData(charId);
    },
    [selectCharacter, loadCharacterData],
  );

  const loadDatabase = useCallback(
    async (path: string) => {
      await openDatabase(path);
      setDbPath(path);
      localStorage.setItem(STORAGE_KEYS.LAST_DB, path);
      const chars = await listCharacters();
      setCharacters(chars);
      const count = await getScannedLogCount();
      setScannedLogCount(count);
      const lineCount = await getLogLineCount();
      setLogLineCount(lineCount);
      const logs = await getProcessLogs();
      setProcessLogs(logs);
      if (chars.length > 0) {
        const lastCharId = Number(localStorage.getItem(STORAGE_KEYS.LAST_CHARACTER));
        const toSelect = chars.find((c) => c.id === lastCharId) ?? chars[0];
        if (toSelect.id !== null) {
          await handleSelectCharacter(toSelect.id);
        }
      }
    },
    [setDbPath, setCharacters, setScannedLogCount, setLogLineCount, setProcessLogs, handleSelectCharacter],
  );

  // Auto-open database on startup: last-used path, or default app data dir
  useEffect(() => {
    const lastDb = localStorage.getItem(STORAGE_KEYS.LAST_DB);
    if (lastDb) {
      loadDatabase(lastDb).catch(() => {
        localStorage.removeItem(STORAGE_KEYS.LAST_DB);
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
      try {
        await loadDatabase(selected);
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
    localStorage.setItem(STORAGE_KEYS.LAST_DB, defaultPath);
    return true;
  }, [dbPath, setDbPath]);

  const handleReset = useCallback(async () => {
    if (!dbPath) return;
    const confirmed = await confirm(
      "This will clear all scanned data (kills, trainers, pets, lastys) and reset all stats. Your rank modifier settings will be preserved.\n\nAre you sure?",
      { title: "Reset Database", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      await resetDatabase();
      const chars = await listCharacters();
      setCharacters(chars);
      const count = await getScannedLogCount();
      setScannedLogCount(count);
      setLogLineCount(0);
      if (chars.length > 0 && chars[0].id !== null) {
        await handleSelectCharacter(chars[0].id);
      }
    } catch (e) {
      console.error("Reset failed:", e);
    }
  }, [dbPath, setCharacters, setScannedLogCount, setLogLineCount, handleSelectCharacter]);

  const handleDeleteAll = useCallback(async () => {
    if (!dbPath) return;
    const confirmed = await confirm(
      "This will permanently delete ALL data: characters, trainers, kills, pets, logs, and rank modifiers.\n\nThis cannot be undone. Are you sure?",
      { title: "Delete All Data", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      await deleteAllData();
      setCharacters([]);
      setScannedLogCount(0);
      setLogLineCount(0);
      setKills([]);
      setTrainers([]);
      setPets([]);
      setLastys([]);
      setProcessLogs([]);
    } catch (e) {
      console.error("Delete all data failed:", e);
    }
  }, [dbPath, setCharacters, setScannedLogCount, setLogLineCount, setKills, setTrainers, setPets, setLastys, setProcessLogs]);

  const handleImportScribius = useCallback(async () => {
    const scribiusFile = await open({
      filters: [{ name: "Scribius Database", extensions: ["db", "sqlite"] }],
      title: "Select Scribius Database",
    });
    if (!scribiusFile) return;

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

  return {
    loadDatabase,
    handleOpenDb,
    handleReset,
    handleDeleteAll,
    handleImportScribius,
    handleSelectCharacter,
    ensureDb,
    selectedCharacterId,
    isScanning,
  };
}
