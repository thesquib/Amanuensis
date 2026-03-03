import { useState } from "react";
import { useStore } from "../../lib/store";
import { useDatabase } from "../../lib/hooks/useDatabase";
import { useScan } from "../../lib/hooks/useScan";
import { revealDatabase } from "../../lib/commands";
import { ProgressBar } from "../shared/ProgressBar";
import { CharacterList } from "./CharacterList";
import type { Theme } from "../../lib/store";

export function Sidebar() {
  const { dbPath, logFolder, scannedLogCount, recursiveScan, setRecursiveScan, indexLogLines, setIndexLogLines, theme, setTheme } = useStore();

  const [advancedOpen, setAdvancedOpen] = useState(false);

  const { handleOpenDb, handleReset, handleImportScribius, handleSelectCharacter, ensureDb, isScanning } = useDatabase();

  const { scanProgress, handleScanFolder, handleScanFiles, handleRescanLogs } = useScan(
    async (chars) => {
      if (chars.length > 0 && chars[0].id !== null) {
        await handleSelectCharacter(chars[0].id);
      }
    },
  );

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
        <button onClick={handleOpenDb} disabled={isScanning} className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80 disabled:opacity-50">
          Open Database
        </button>
        <button onClick={() => handleScanFolder(ensureDb)} disabled={isScanning} className="rounded bg-[var(--color-accent)] px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/80 disabled:opacity-50">
          {isScanning ? "Scanning..." : "Scan Log Folder(s)"}
        </button>
        <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
          <input type="checkbox" checked={recursiveScan} onChange={(e) => setRecursiveScan(e.target.checked)} disabled={isScanning} className="accent-[var(--color-accent)]" />
          Deep scan (find logs recursively)
        </label>
        {/* Advanced section */}
        <button
          onClick={() => setAdvancedOpen((o) => !o)}
          className="flex items-center gap-1 text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text)] mt-1"
        >
          <span className={`transition-transform ${advancedOpen ? "rotate-90" : ""}`}>▶</span>
          Advanced
        </button>
        {advancedOpen && (
          <div className="flex flex-col gap-2">
            <button onClick={() => handleScanFiles(ensureDb)} disabled={isScanning} className="rounded bg-[var(--color-accent)]/80 px-3 py-1.5 text-sm font-medium text-white hover:bg-[var(--color-accent)]/60 disabled:opacity-50">
              Scan Specific Log Files
            </button>
            <button
              onClick={handleRescanLogs}
              disabled={isScanning || !logFolder}
              className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80 disabled:opacity-50"
              title={logFolder ? "Clear all scanned data and rescan from scratch (preserves rank modifiers)" : "No log folder selected — scan a folder first"}
            >
              Rescan Logs
            </button>
            <button onClick={handleImportScribius} disabled={isScanning} className="rounded border border-[var(--color-border)] bg-[var(--color-btn-secondary)] px-3 py-1.5 text-sm font-medium hover:opacity-80 disabled:opacity-50">
              Import Scribius DB
            </button>
            <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
              <input type="checkbox" checked={indexLogLines} onChange={(e) => setIndexLogLines(e.target.checked)} disabled={isScanning} className="accent-[var(--color-accent)]" />
              Index logs for search
            </label>
            <div className="text-[10px] text-[var(--color-text-muted)]/60 leading-tight">
              Characters are detected automatically from log content. Mixed-character folders are supported.
            </div>
            {dbPath && (
              <div className="text-xs text-[var(--color-text-muted)]">
                <div className="flex items-center gap-1">
                  <div className="min-w-0 flex-1 truncate" title={dbPath}>
                    DB: {dbPath.split("/").pop()}
                  </div>
                  <button
                    onClick={() => revealDatabase(dbPath)}
                    title="Show in Finder"
                    className="shrink-0 rounded px-1 py-0.5 text-[10px] text-[var(--color-text-muted)] hover:bg-[var(--color-card)] hover:text-[var(--color-text)]"
                  >
                    Reveal
                  </button>
                </div>
                {logFolder && <div className="mt-1 truncate" title={logFolder}>Logs: {logFolder.split("/").pop()}</div>}
                <div className="mt-1">{scannedLogCount} files scanned</div>
                <button
                  onClick={handleReset}
                  disabled={isScanning}
                  className="mt-2 w-full rounded px-2 py-1 text-xs disabled:opacity-50"
                  style={{ backgroundColor: "var(--color-danger-bg)", color: "var(--color-danger)" }}
                  onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "var(--color-danger-bg-hover)")}
                  onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "var(--color-danger-bg)")}
                >
                  Reset Database
                </button>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Scan progress */}
      {isScanning && scanProgress && (
        <div className="border-b border-[var(--color-border)] p-3">
          <ProgressBar current={scanProgress.current_file} total={scanProgress.total_files} label={scanProgress.filename} />
        </div>
      )}

      {/* Character list */}
      <CharacterList onSelectCharacter={handleSelectCharacter} />
    </div>
  );
}
