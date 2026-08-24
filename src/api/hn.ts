import { Array as Arr, Cache, Data, Duration, Effect, Schedule } from "effect"
import type { FeedCategory, Item } from "./types"

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

export type HnError = HnRequestError | HnStatusError | HnTimeoutError

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

// ---------------------------------------------------------------------------
// Feed ids
// ---------------------------------------------------------------------------

export const fetchIdsEffect = (
  category: FeedCategory,
): Effect.Effect<number[], HnError> =>
  getJson(`${category}stories.json`).pipe(
    Effect.map((ids) => (ids as number[] | null) ?? []),
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
    lookup: (id: number) =>
      getJson(`item/${id}.json`).pipe(Effect.map((raw) => raw as Item)),
  }),
)

export const fetchItemEffect = (id: number): Effect.Effect<Item, HnError> =>
  itemCache.get(id).pipe(Effect.tapError(() => itemCache.invalidate(id)))

// Replaces the hand-rolled worker pool: fetch many items, at most
// `concurrency` in flight, failures dropped, input order preserved.
export const fetchItemsEffect = (
  ids: ReadonlyArray<number>,
  concurrency = 10,
): Effect.Effect<Item[], never> =>
  Effect.forEach(ids, (id) => fetchItemEffect(id).pipe(Effect.option), {
    concurrency,
  }).pipe(
    // getSomes drops the failures; filter(Boolean) drops null items —
    // HN returns `null` with a 200 for deleted/nonexistent ids
    Effect.map((opts) => Arr.getSomes(opts).filter((x): x is Item => Boolean(x))),
  )

// ---------------------------------------------------------------------------
// Promise adapters — the hooks still speak Promise for now. These are the
// only places we *run* effects; everything above merely describes them.
// ---------------------------------------------------------------------------

export function fetchIds(category: FeedCategory): Promise<number[]> {
  return Effect.runPromise(fetchIdsEffect(category))
}

export function fetchItem(id: number): Promise<Item> {
  return Effect.runPromise(fetchItemEffect(id))
}

export function fetchItems(ids: number[], concurrency = 10): Promise<Item[]> {
  return Effect.runPromise(fetchItemsEffect(ids, concurrency))
}
