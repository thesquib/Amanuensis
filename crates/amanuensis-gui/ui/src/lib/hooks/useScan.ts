import { open, confirm } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useCallback } from "react";
import { useStore } from "../store";
import { scanLogs, rescanLogs, scanFiles, listCharacters, getScannedLogCount, getLogLineCount, getProcessLogs } from "../commands";
import type { ScanProgress } from "../../types";

export function useScan(onScanComplete: (chars: Awaited<ReturnType<typeof listCharacters>>) => Promise<void>) {
  const {
    logFolder,
    setLogFolder,
    isScanning,
    setIsScanning,
    scanProgress,
    setScanProgress,
    setCharacters,
    setScannedLogCount,
    setLogLineCount,
    setProcessLogs,
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
    await onScanComplete(chars);
  }, [setCharacters, setScannedLogCount, setLogLineCount, setProcessLogs, onScanComplete]);

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
  }, [setLogFolder, setIsScanning, setScanProgress, finishScan, recursiveScan, indexLogLines]);

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
    if (!logFolder) return;
    const confirmed = await confirm(
      "This will clear all scanned data and rescan your logs from scratch. Your rank modifier settings will be preserved. Continue?",
      { title: "Rescan Logs", kind: "warning" },
    );
    if (!confirmed) return;
    setIsScanning(true);
    setScanProgress(null);
    try {
      await rescanLogs(logFolder, recursiveScan, indexLogLines);
      await finishScan();
    } catch (e) {
      console.error("Rescan failed:", e);
    } finally {
      setIsScanning(false);
      setScanProgress(null);
    }
  }, [logFolder, recursiveScan, indexLogLines, setIsScanning, setScanProgress, finishScan]);

  return {
    isScanning,
    scanProgress,
    logFolder,
    handleScanFolder,
    handleScanFiles,
    handleRescanLogs,
  };
}
