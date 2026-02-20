import { invoke } from "@tauri-apps/api/core";
import type {
  Character,
  Kill,
  Trainer,
  Pet,
  Lasty,
  ScanResult,
  TrainerInfo,
  ImportResult,
  LogSearchResult,
} from "../types";

export async function openDatabase(path: string): Promise<void> {
  return invoke("open_database", { path });
}

export async function listCharacters(): Promise<Character[]> {
  return invoke("list_characters");
}

export async function getCharacter(name: string): Promise<Character | null> {
  return invoke("get_character", { name });
}

export async function getKills(charId: number): Promise<Kill[]> {
  return invoke("get_kills", { charId });
}

export async function getTrainers(charId: number): Promise<Trainer[]> {
  return invoke("get_trainers", { charId });
}

export async function getPets(charId: number): Promise<Pet[]> {
  return invoke("get_pets", { charId });
}

export async function getLastys(charId: number): Promise<Lasty[]> {
  return invoke("get_lastys", { charId });
}

export async function getScannedLogCount(): Promise<number> {
  return invoke("get_scanned_log_count");
}

export async function getTrainerDbInfo(): Promise<TrainerInfo[]> {
  return invoke("get_trainer_db_info");
}

export async function setModifiedRanks(
  charId: number,
  trainerName: string,
  modifiedRanks: number,
): Promise<void> {
  return invoke("set_modified_ranks", { charId, trainerName, modifiedRanks });
}

export async function scanLogs(
  folder: string,
  force: boolean,
  recursive: boolean = false,
  indexLines: boolean = true,
): Promise<ScanResult> {
  return invoke("scan_logs", { folder, force, recursive, indexLines });
}

export async function scanFiles(
  files: string[],
  force: boolean = false,
  indexLines: boolean = true,
): Promise<ScanResult> {
  return invoke("scan_files", { files, force, indexLines });
}

export async function searchLogs(
  query: string,
  charId?: number | null,
  limit?: number,
): Promise<LogSearchResult[]> {
  return invoke("search_logs", { query, charId: charId ?? null, limit: limit ?? 200 });
}

export async function getLogLineCount(): Promise<number> {
  return invoke("get_log_line_count");
}

export async function getDefaultDbPath(): Promise<string> {
  return invoke("get_default_db_path");
}

export async function checkDbExists(path: string): Promise<boolean> {
  return invoke("check_db_exists", { path });
}

export async function resetDatabase(): Promise<void> {
  return invoke("reset_database");
}

export async function importScribiusDb(
  scribiusPath: string,
  outputPath: string,
  force: boolean = false,
): Promise<ImportResult> {
  return invoke("import_scribius_db", { scribiusPath, outputPath, force });
}
