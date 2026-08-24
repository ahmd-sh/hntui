import { useEffect, useState } from "react"
import { Effect, Fiber } from "effect"
import { fetchIds } from "../api/hn"
import type { HnError } from "../api/hn"
import type { FeedCategory } from "../api/types"

export function useStoryIds(category: FeedCategory, refreshKey = 0) {
  const [ids, setIds] = useState<number[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<HnError | null>(null)

  useEffect(() => {
    setIds([])
    setLoading(true)
    setError(null)
    const fiber = Effect.runFork(
      fetchIds(category).pipe(
        Effect.match({
          onSuccess: (data) => {
            setIds(data)
            setLoading(false)
          },
          onFailure: (err) => {
            setError(err)
            setLoading(false)
          },
        }),
      ),
    )
    return () => {
      // interrupting the fiber aborts the in-flight HTTP request
      Effect.runFork(Fiber.interrupt(fiber))
    }
  }, [category, refreshKey])

  return { ids, loading, error }
}
