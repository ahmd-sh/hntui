import { join } from "path"
import { mkdirSync } from "fs"
import { configDir } from "./configDir"

export interface SavedEntry {
  id: number
  savedAt: number
}

const SAVED_PATH = join(configDir(), "saved.json")

export async function loadSaved(): Promise<SavedEntry[]> {
  try {
    const file = Bun.file(SAVED_PATH)
    if (!(await file.exists())) return []
    const data = await file.json()
    if (!Array.isArray(data)) return []
    return data.filter(
      (e): e is SavedEntry =>
        e && typeof e.id === "number" && typeof e.savedAt === "number",
    )
  } catch {
    return []
  }
}

export async function persistSaved(entries: SavedEntry[]): Promise<void> {
  try {
    mkdirSync(configDir(), { recursive: true })
    await Bun.write(SAVED_PATH, JSON.stringify(entries, null, 2))
  } catch {
    // fail silently — saved state is best-effort
  }
}
