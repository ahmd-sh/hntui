import { Array as Arr, Cache, Context, Data, Duration, Effect, Layer, Schedule, Schema } from "effect"
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

// Not an API failure — a link resolved to nothing viewable.
export class HnItemGone extends Data.TaggedError("HnItemGone")<{
  readonly id: number
}> {}

export interface ResolvedLink {
  story: Item
  // the comment the link pointed at, to focus after opening (best effort)
  focusId?: number
}

// ---------------------------------------------------------------------------
// HnTransport — the service that produces raw JSON. This is the app's only
// contact with the outside world; everything above it is policy and domain
// logic. Swapping this layer swaps the data source (live / demo / test).
// ---------------------------------------------------------------------------

export interface HnTransportShape {
  readonly getJson: (path: string) => Effect.Effect<unknown, HnError>
}

export class HnTransport extends Context.Tag("HnTransport")<HnTransport, HnTransportShape>() {}

// The live transport: fetch against Firebase. Timeout aborts the underlying
// request (via the injected AbortSignal); transient failures retry with
// exponential backoff, client errors don't.
export const makeLiveGetJson =
  (): HnTransportShape["getJson"] =>
  (path) =>
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
        while: (e) => e._tag !== "HnStatusError" || e.status >= 500,
      }),
    )

export const HnTransportLive = Layer.succeed(HnTransport, { getJson: makeLiveGetJson() })

// ---------------------------------------------------------------------------
// HnApi — the domain service: feeds, items, link resolution. Its layer asks
// for a transport and builds the item cache, so the cache lives exactly as
// long as the runtime that built the layer — not "forever, from module
// import" like the old module-global Effect.runSync version.
// ---------------------------------------------------------------------------

export interface HnApiShape {
  readonly fetchIds: (category: FeedCategory) => Effect.Effect<number[], HnError>
  readonly fetchItem: (id: number) => Effect.Effect<Item | null, HnError>
  readonly fetchItems: (ids: ReadonlyArray<number>, concurrency?: number) => Effect.Effect<Item[]>
  readonly resolveStory: (ref: HnItemRef) => Effect.Effect<ResolvedLink, HnError | HnItemGone>
}

export class HnApi extends Context.Tag("HnApi")<HnApi, HnApiShape>() {}

// HN returns `null` (with a 200) for deleted/nonexistent ids — the schemas
// say so explicitly instead of a cast papering over it.
const IdsPayload = Schema.NullOr(Schema.mutable(Schema.Array(Schema.Number)))
const ItemPayload = Schema.NullOr(ItemSchema)

const isComment = (i: Item) => i.type === "comment" || i.type === "pollopt"

export const HnApiLive = Layer.effect(
  HnApi,
  Effect.gen(function* () {
    const transport = yield* HnTransport

    // GET + validate: the `unknown` from the wire only becomes an A by
    // passing through the schema.
    const getDecoded = <A, I>(
      path: string,
      schema: Schema.Schema<A, I, never>,
    ): Effect.Effect<A, HnError> =>
      transport.getJson(path).pipe(
        Effect.flatMap(Schema.decodeUnknown(schema)),
        Effect.catchTag("ParseError", (e) =>
          Effect.fail(new HnDecodeError({ path, issue: e.message })),
        ),
      )

    // Concurrent lookups of the same id share one request, successes are
    // memoized, failures are evicted so the next attempt refetches.
    const cache = yield* Cache.make({
      capacity: 50_000,
      timeToLive: Duration.infinity,
      lookup: (id: number) => getDecoded(`item/${id}.json`, ItemPayload),
    })

    const fetchIds: HnApiShape["fetchIds"] = (category) =>
      getDecoded(`${category}stories.json`, IdsPayload).pipe(Effect.map((ids) => ids ?? []))

    const fetchItem: HnApiShape["fetchItem"] = (id) =>
      cache.get(id).pipe(Effect.tapError(() => cache.invalidate(id)))

    // Fetch many items, at most `concurrency` in flight, failures and null
    // items dropped, input order preserved.
    const fetchItems: HnApiShape["fetchItems"] = (ids, concurrency = 10) =>
      Effect.forEach(ids, (id) => fetchItem(id).pipe(Effect.option), {
        concurrency,
      }).pipe(
        Effect.map((opts) => Arr.getSomes(opts).filter((x): x is Item => x !== null)),
      )

    // An /item?id=N link may point at a comment: walk `parent` upward until
    // the root story. Every hop inherits retry/timeout/caching from
    // fetchItem, and interruption aborts mid-chain.
    const resolveStory: HnApiShape["resolveStory"] = (ref) =>
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

    return HnApi.of({ fetchIds, fetchItem, fetchItems, resolveStory })
  }),
)

// ---------------------------------------------------------------------------
// Accessors — call sites keep the exact shape they had. The only change is
// in the type: `HnApi` in the R channel names the dependency, and the
// compiler refuses to run these until something provides it.
// ---------------------------------------------------------------------------

export const fetchIds = (category: FeedCategory): Effect.Effect<number[], HnError, HnApi> =>
  Effect.flatMap(HnApi, (api) => api.fetchIds(category))

export const fetchItem = (id: number): Effect.Effect<Item | null, HnError, HnApi> =>
  Effect.flatMap(HnApi, (api) => api.fetchItem(id))

export const fetchItems = (
  ids: ReadonlyArray<number>,
  concurrency = 10,
): Effect.Effect<Item[], never, HnApi> =>
  Effect.flatMap(HnApi, (api) => api.fetchItems(ids, concurrency))

export const resolveStory = (
  ref: HnItemRef,
): Effect.Effect<ResolvedLink, HnError | HnItemGone, HnApi> =>
  Effect.flatMap(HnApi, (api) => api.resolveStory(ref))
