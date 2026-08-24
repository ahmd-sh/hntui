import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { HistoryEntry } from "../utils/historyStore"
import { HISTORY_CAP, loadHistory, persistHistory } from "../utils/historyStore"

export function useHistory() {
  const [entries, setEntries] = useState<HistoryEntry[]>([])
  const [loaded, setLoaded] = useState(false)
  const firstLoad = useRef(true)

  useEffect(() => {
    loadHistory().then((data) => {
      setEntries(data)
      setLoaded(true)
    })
  }, [])

  useEffect(() => {
    if (firstLoad.current) {
      firstLoad.current = false
      return
    }
    if (loaded) void persistHistory(entries)
  }, [entries, loaded])

  const idSet = useMemo(() => new Set(entries.map((e) => e.id)), [entries])

  const isViewed = useCallback((id: number) => idSet.has(id), [idSet])

  // Move the post to the front (most recent) and evict the oldest beyond the cap.
  const markViewed = useCallback((id: number) => {
    setEntries((prev) => {
      const rest = prev.filter((e) => e.id !== id)
      return [{ id, viewedAt: Date.now() }, ...rest].slice(0, HISTORY_CAP)
    })
  }, [])

  const clear = useCallback(() => setEntries([]), [])

  return { entries, idSet, isViewed, markViewed, clear, loaded }
}
