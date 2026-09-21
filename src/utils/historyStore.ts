import { join } from "path"
import { mkdirSync } from "fs"
import { configDir } from "./configDir"

export interface HistoryEntry {
  id: number
  viewedAt: number
}

export const HISTORY_CAP = 1000

const HISTORY_PATH = join(configDir(), "history.json")

export async function loadHistory(): Promise<HistoryEntry[]> {
  try {
    const file = Bun.file(HISTORY_PATH)
    if (!(await file.exists())) return []
    const data = await file.json()
    if (!Array.isArray(data)) return []
    return data
      .filter(
        (e): e is HistoryEntry =>
          e && typeof e.id === "number" && typeof e.viewedAt === "number",
      )
      .slice(0, HISTORY_CAP)
  } catch {
    return []
  }
}

export async function persistHistory(entries: HistoryEntry[]): Promise<void> {
  try {
    mkdirSync(configDir(), { recursive: true })
    await Bun.write(HISTORY_PATH, JSON.stringify(entries, null, 2))
  } catch {
    // fail silently — view history is best-effort
  }
}
