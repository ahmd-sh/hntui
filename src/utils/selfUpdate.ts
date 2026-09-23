import { chmodSync, mkdtempSync, renameSync, rmSync } from "fs"
import { dirname, join } from "path"
import { VERSION } from "../version"

const REPO = "ahmd-sh/hntui"
const LATEST_URL = `https://github.com/${REPO}/releases/latest`

// "1.2.3" vs "1.10.0" → negative if a < b, 0 if equal, positive if a > b
export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number)
  const pb = b.split(".").map(Number)
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0)
    if (d !== 0) return d
  }
  return 0
}

// which release tarball serves this machine, or null if none exists
export function releaseAsset(platform: string, arch: string): string | null {
  if (platform === "darwin" && arch === "arm64") return "hntui-darwin-arm64.tar.gz"
  if (platform === "linux" && (arch === "x64" || arch === "arm64"))
    return `hntui-linux-${arch}.tar.gz`
  return null
}

// The releases/latest page redirects to .../tag/vX.Y.Z — the version is in a
// header of a tiny response: no API, no auth, no rate-limit concerns.
export async function fetchLatestVersion(timeoutMs = 5000): Promise<string | null> {
  try {
    const res = await fetch(LATEST_URL, {
      redirect: "manual",
      signal: AbortSignal.timeout(timeoutMs),
    })
    const location = res.headers.get("location") ?? ""
    const m = location.match(/\/tag\/v(\d+\.\d+\.\d+)$/)
    return m ? m[1]! : null
  } catch {
    return null
  }
}

type Channel = "binary" | "bun" | "dev"

// A compiled executable runs from bun's virtual filesystem; under the bun
// runtime, a global install lives in node_modules, a source checkout doesn't.
function installChannel(): Channel {
  if (import.meta.url.includes("$bunfs")) return "binary"
  return import.meta.path.includes("node_modules") ? "bun" : "dev"
}

// `hntui update` — returns a process exit code.
export async function selfUpdate(): Promise<number> {
  const latest = await fetchLatestVersion()
  if (!latest) {
    console.error("hntui: couldn't reach GitHub to check the latest release.")
    return 1
  }
  if (compareVersions(latest, VERSION) <= 0) {
    console.log(`hntui ${VERSION} is up to date (latest release is v${latest}).`)
    return 0
  }

  console.log(`hntui ${VERSION} → v${latest} available.`)
  const channel = installChannel()
  if (channel === "bun") {
    console.log("This copy was installed with Bun — update it with:\n  bun add -g @ahmd-sh/hntui")
    return 0
  }
  if (channel === "dev") {
    console.log("Running from a source checkout — update it with git pull.")
    return 0
  }

  const asset = releaseAsset(process.platform, process.arch)
  if (!asset) {
    console.error(
      `hntui: no prebuilt binary for ${process.platform}-${process.arch}.\n` +
        "Install with Bun instead:  bun add -g @ahmd-sh/hntui",
    )
    return 1
  }

  const target = process.execPath
  const targetDir = dirname(target)
  console.log(`downloading ${asset} ...`)
  const res = await fetch(`https://github.com/${REPO}/releases/latest/download/${asset}`)
  if (!res.ok) {
    console.error(`hntui: download failed (HTTP ${res.status}).`)
    return 1
  }

  // extract in the target's own directory: the final rename must not cross
  // filesystems, and renaming over a running executable is safe (the running
  // process keeps its inode; the path points at the new file)
  let tmpDir: string | null = null
  try {
    tmpDir = mkdtempSync(join(targetDir, ".hntui-update-"))
    const tarPath = join(tmpDir, asset)
    await Bun.write(tarPath, res)
    const tar = Bun.spawn(["tar", "-xzf", tarPath, "-C", tmpDir], {
      stdout: "ignore",
      stderr: "pipe",
    })
    if ((await tar.exited) !== 0) {
      console.error(`hntui: extract failed: ${await new Response(tar.stderr).text()}`)
      return 1
    }
    const fresh = join(tmpDir, "hntui")
    chmodSync(fresh, 0o755)
    renameSync(fresh, target)
    console.log(`updated to v${latest}  (${target})`)
    return 0
  } catch (err) {
    console.error(`hntui: update failed: ${err}\nIs ${targetDir} writable?`)
    return 1
  } finally {
    if (tmpDir) rmSync(tmpDir, { recursive: true, force: true })
  }
}
