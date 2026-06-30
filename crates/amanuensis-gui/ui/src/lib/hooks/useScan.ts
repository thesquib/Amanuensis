import { open, confirm } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useCallback } from "react";
import { useStore } from "../store";
import { scanLogs, rescanLogs, scanFiles, updateLogs, getPendingLogCount, listCharacters, getScannedLogCount, getLogLineCount, getProcessLogs } from "../commands";
import type { ScanProgress } from "../../types";

export function useScan(onScanComplete: (chars: Awaited<ReturnType<typeof listCharacters>>) => Promise<void>) {
  const {
    sources,
    addSource,
    isScanning,
    setIsScanning,
    scanProgress,
    setScanProgress,
    characters,
    setCharacters,
    setScannedLogCount,
    setLogLineCount,
    setProcessLogs,
    setPendingLogCount,
    setUpdateResult,
    recursiveScan,
    indexLogLines,
  } = useStore();

  const finishScan = useCallback(async () => {
    const chars = await listCharacters();
    setCharacters(chars);
    const count = await getScannedLogCount();
    setScannedLogCount(count);
    const lineCount = await getLogLineCount();
    setLogLineCount(lineCount);
    const logs = await getProcessLogs();
    setProcessLogs(logs);
    const pending = await getPendingLogCount(sources);
    setPendingLogCount(pending);
    await onScanComplete(chars);
  }, [setCharacters, setScannedLogCount, setLogLineCount, setProcessLogs, sources, setPendingLogCount, onScanComplete]);

  // Listen for scan progress events
  useEffect(() => {
    const unlisten = listen<ScanProgress>("scan-progress", (event) => {
      setScanProgress(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setScanProgress]);

  const handleScanFolder = useCallback(async (ensureDb: () => Promise<boolean>) => {
    if (!(await ensureDb())) return;

    const folder = await open({ directory: true, recursive: true, title: "Select Log Folder" });
    if (!folder) return;
    const folderPath = typeof folder === "string" ? folder : folder;

    setIsScanning(true);
    setScanProgress(null);

    try {
      await scanLogs(folderPath, false, recursiveScan, indexLogLines);
      addSource(folderPath, recursiveScan);
      await finishScan();
    } catch (e) {
      console.error("Scan failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [addSource, setIsScanning, setScanProgress, finishScan, recursiveScan, indexLogLines]);

  const handleScanFiles = useCallback(async (ensureDb: () => Promise<boolean>) => {
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
  }, [setIsScanning, setScanProgress, finishScan, indexLogLines]);

  const handleRescanLogs = useCallback(async () => {
    if (sources.length === 0) return;
    const confirmed = await confirm(
      "This will clear all scanned data and rescan every remembered log source from scratch. Your rank modifier settings will be preserved. Continue?",
      { title: "Rescan Logs", kind: "warning" },
    );
    if (!confirmed) return;
    setIsScanning(true);
    setScanProgress(null);
    try {
      await rescanLogs(sources, indexLogLines);
      await finishScan();
    } catch (e) {
      console.error("Rescan failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [sources, indexLogLines, setIsScanning, setScanProgress, finishScan]);

  const handleUpdateLogs = useCallback(async () => {
    if (sources.length === 0) return;
    setIsScanning(true);
    setScanProgress(null);
    try {
      // Snapshot per-character stats before the run so we can report the deltas.
      const before = new Map(characters.map((c) => [c.name, c]));
      const scan = await updateLogs(sources, indexLogLines);
      await finishScan();
      const after = await listCharacters();
      const perCharacter = after
        .map((c) => ({
          name: c.name,
          loginsDelta: c.logins - (before.get(c.name)?.logins ?? 0),
          deathsDelta: c.deaths - (before.get(c.name)?.deaths ?? 0),
        }))
        .filter((d) => d.loginsDelta !== 0 || d.deathsDelta !== 0)
        .sort((a, b) => b.loginsDelta - a.loginsDelta);
      setUpdateResult({ scan, perCharacter });
    } catch (e) {
      console.error("Update logs failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [sources, indexLogLines, characters, setIsScanning, setScanProgress, finishScan, setUpdateResult]);

  return {
    isScanning,
    scanProgress,
    sources,
    handleScanFolder,
    handleScanFiles,
    handleRescanLogs,
    handleUpdateLogs,
  };
}
