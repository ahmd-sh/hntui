import { homedir } from "os"
import { join } from "path"
import { existsSync, renameSync } from "fs"

const OLD_DIR = join(homedir(), ".config", "hackernuis")
const NEW_DIR = join(homedir(), ".config", "hntui")

let checked = false

// The app was renamed from hackernuis to hntui: move existing config
// (saved posts, view history) to the new location. Once, best-effort.
export function configDir(): string {
  if (!checked) {
    checked = true
    try {
      if (!existsSync(NEW_DIR) && existsSync(OLD_DIR)) renameSync(OLD_DIR, NEW_DIR)
    } catch {
      // fall through — the stores create the dir on write anyway
    }
  }
  return NEW_DIR
}
