import { useEffect, useState } from "react"
import { Effect, Fiber } from "effect"
import { fetchItems } from "../api/hn"
import type { Item } from "../api/types"

export function useItems(ids: number[]) {
  const [items, setItems] = useState<Item[]>([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (ids.length === 0) {
      setItems([])
      return
    }
    setItems([])
    setLoading(true)
    const fiber = Effect.runFork(
      fetchItems(ids).pipe(
        Effect.andThen((data) =>
          Effect.sync(() => {
            setItems(data)
            setLoading(false)
          }),
        ),
      ),
    )
    return () => {
      Effect.runFork(Fiber.interrupt(fiber))
    }
  }, [ids.join(",")])

  return { items, loading }
}
