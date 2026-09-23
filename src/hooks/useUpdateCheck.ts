import { useEffect, useState } from "react"
import { Effect, Fiber } from "effect"
import { AppRuntime } from "../runtime"
import { VERSION } from "../version"
import { compareVersions, fetchLatestVersion } from "../utils/selfUpdate"

// One check per app launch, silently skipped when offline: returns the newer
// version string when a release is ahead of this build, otherwise null.
export function useUpdateCheck(): string | null {
  const [latest, setLatest] = useState<string | null>(null)

  useEffect(() => {
    const fiber = AppRuntime.runFork(
      // fetchLatestVersion never throws (5s timeout, null on any failure)
      Effect.promise(() => fetchLatestVersion()).pipe(
        Effect.andThen((v) =>
          Effect.sync(() => {
            if (v && compareVersions(v, VERSION) > 0) setLatest(v)
          }),
        ),
      ),
    )
    return () => {
      AppRuntime.runFork(Fiber.interrupt(fiber))
    }
  }, [])

  return latest
}
