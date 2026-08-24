import { Array as Arr, Cache, Data, Duration, Effect, Schedule, Schema } from "effect"
import { ItemSchema } from "./types"
import type { FeedCategory, Item } from "./types"
import type { HnItemRef } from "../utils/format"

const BASE = "https://hacker-news.firebaseio.com/v0"

// ---------------------------------------------------------------------------
// Errors — every way a call can fail, as its own named type.
// These show up in the *type* of each effect below.
// ---------------------------------------------------------------------------

export class HnRequestError extends Data.TaggedError("HnRequestError")<{
  readonly path: string
  readonly cause: unknown
}> {}

export class HnStatusError extends Data.TaggedError("HnStatusError")<{
  readonly path: string
  readonly status: number
}> {}

export class HnTimeoutError extends Data.TaggedError("HnTimeoutError")<{
  readonly path: string
}> {}

export class HnDecodeError extends Data.TaggedError("HnDecodeError")<{
  readonly path: string
  readonly issue: string
}> {}

export type HnError = HnRequestError | HnStatusError | HnTimeoutError | HnDecodeError

// ---------------------------------------------------------------------------
// The one HTTP building block: GET a JSON document from the HN API.
// Timeout aborts the underlying fetch (via the injected AbortSignal);
// transient failures are retried with exponential backoff.
// ---------------------------------------------------------------------------

const getJson = (path: string): Effect.Effect<unknown, HnError> =>
  Effect.gen(function* () {
    const res = yield* Effect.tryPromise({
      try: (signal) => fetch(`${BASE}/${path}`, { signal }),
      catch: (cause) => new HnRequestError({ path, cause }),
    })
    if (!res.ok) {
      return yield* new HnStatusError({ path, status: res.status })
    }
    return yield* Effect.tryPromise({
      try: () => res.json() as Promise<unknown>,
      catch: (cause) => new HnRequestError({ path, cause }),
    })
  }).pipe(
    Effect.timeoutFail({
      duration: "5 seconds",
      onTimeout: () => new HnTimeoutError({ path }),
    }),
    Effect.retry({
      schedule: Schedule.exponential("250 millis"),
      times: 2,
      // don't retry client errors (404 etc.) — only network/timeout/5xx
      while: (e) => e._tag !== "HnStatusError" || e.status >= 500,
    }),
  )

// GET + validate: the `unknown` from the wire only becomes an A by passing
// through the schema. A payload that doesn't match fails with HnDecodeError.
const getDecoded = <A, I>(
  path: string,
  schema: Schema.Schema<A, I, never>,
): Effect.Effect<A, HnError> =>
  getJson(path).pipe(
    Effect.flatMap(Schema.decodeUnknown(schema)),
    Effect.catchTag("ParseError", (e) =>
      Effect.fail(new HnDecodeError({ path, issue: e.message })),
    ),
  )

// HN returns `null` (with a 200) for deleted/nonexistent ids — the schemas
// say so explicitly instead of a cast papering over it.
const IdsPayload = Schema.NullOr(Schema.mutable(Schema.Array(Schema.Number)))
const ItemPayload = Schema.NullOr(ItemSchema)

// ---------------------------------------------------------------------------
// Feed ids
// ---------------------------------------------------------------------------

export const fetchIds = (
  category: FeedCategory,
): Effect.Effect<number[], HnError> =>
  getDecoded(`${category}stories.json`, IdsPayload).pipe(
    Effect.map((ids) => ids ?? []),
  )

// ---------------------------------------------------------------------------
// Items — Cache replaces the old `cache` Map AND the `inflight` Map:
// concurrent lookups of the same id share one request, successes are
// memoized. Failures are evicted so the next attempt refetches.
// ---------------------------------------------------------------------------

const itemCache = Effect.runSync(
  Cache.make({
    capacity: 50_000,
    timeToLive: Duration.infinity,
    lookup: (id: number) => getDecoded(`item/${id}.json`, ItemPayload),
  }),
)

export const fetchItem = (id: number): Effect.Effect<Item | null, HnError> =>
  itemCache.get(id).pipe(Effect.tapError(() => itemCache.invalidate(id)))

// Replaces the hand-rolled worker pool: fetch many items, at most
// `concurrency` in flight, failures and null items dropped, order preserved.
export const fetchItems = (
  ids: ReadonlyArray<number>,
  concurrency = 10,
): Effect.Effect<Item[], never> =>
  Effect.forEach(ids, (id) => fetchItem(id).pipe(Effect.option), {
    concurrency,
  }).pipe(
    Effect.map((opts) => Arr.getSomes(opts).filter((x): x is Item => x !== null)),
  )

// ---------------------------------------------------------------------------
// Internal-link resolution: an /item?id=N link may point at a comment, so
// walk `parent` upward until we reach the root story. Every hop inherits
// retry/timeout/caching from fetchItem, and interruption aborts mid-chain.
// ---------------------------------------------------------------------------

// Not an API failure — the link resolved to nothing viewable.
export class HnItemGone extends Data.TaggedError("HnItemGone")<{
  readonly id: number
}> {}

export interface ResolvedLink {
  story: Item
  // the comment the link pointed at, to focus after opening (best effort)
  focusId?: number
}

const isComment = (i: Item) => i.type === "comment" || i.type === "pollopt"

export const resolveStory = (
  ref: HnItemRef,
): Effect.Effect<ResolvedLink, HnError | HnItemGone> =>
  Effect.gen(function* () {
    const gone = () => new HnItemGone({ id: ref.id })
    const first = yield* fetchItem(ref.id)
    if (!first) return yield* gone()
    // deleted comments still carry `parent`, so we can walk through them
    const fromComment = isComment(first) ? first.id : undefined
    let cur: Item = first
    let hops = 0
    while (isComment(cur)) {
      if (cur.parent == null || ++hops > 64) return yield* gone()
      const parent: Item | null = yield* fetchItem(cur.parent)
      if (!parent) return yield* gone()
      cur = parent
    }
    if (cur.deleted || cur.dead) return yield* gone()
    return { story: cur, focusId: ref.anchorId ?? fromComment }
  })
