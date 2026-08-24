import { Schema } from "effect"

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

export const ItemSchema = Schema.Struct({
  id: Schema.Number,
  type: Schema.Literal("story", "comment", "job", "poll", "pollopt"),
  by: Schema.optional(Schema.String),
  time: Schema.optional(Schema.Number),
  text: Schema.optional(Schema.String),
  dead: Schema.optional(Schema.Boolean),
  deleted: Schema.optional(Schema.Boolean),
  parent: Schema.optional(Schema.Number),
  kids: Schema.optional(Schema.mutable(Schema.Array(Schema.Number))),
  url: Schema.optional(Schema.String),
  score: Schema.optional(Schema.Number),
  title: Schema.optional(Schema.String),
  descendants: Schema.optional(Schema.Number),
})

export type Item = typeof ItemSchema.Type
