export type FeedCategory = "top" | "new" | "best" | "ask" | "show" | "job"
export type Category = FeedCategory | "saved" | "history"

export const FEED_CATEGORIES: { key: FeedCategory; label: string }[] = [
  { key: "top", label: "Top" },
  { key: "new", label: "New" },
  { key: "best", label: "Best" },
  { key: "ask", label: "Ask" },
  { key: "show", label: "Show" },
  { key: "job", label: "Jobs" },
]

export const ALL_CATEGORIES: Category[] = [
  ...FEED_CATEGORIES.map((c) => c.key),
  "history",
  "saved",
]

// Backwards-compat alias used in older imports
export const CATEGORIES = FEED_CATEGORIES

export interface Item {
  id: number
  type: "story" | "comment" | "job" | "poll" | "pollopt"
  by?: string
  time?: number
  text?: string
  dead?: boolean
  deleted?: boolean
  parent?: number
  kids?: number[]
  url?: string
  score?: number
  title?: string
  descendants?: number
}
