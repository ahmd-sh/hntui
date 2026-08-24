import { Effect, Layer } from "effect"
import { HnTransport, makeLiveGetJson } from "./hn"
import type { HnTransportShape } from "./hn"

// Demo transport: the live data source, plus one synthetic comment spliced
// into the first story of the first feed the app loads. This is the typed
// replacement for the old globalThis.fetch monkeypatch preload — select it
// with HN_DEMO=1 (see src/runtime.ts). Real HN is never touched.
//
//   HN_DEMO=1 bun src/index.tsx
//
// The comment carries four links:
//   1. a real HN story link   → opens in the TUI (item 1, pg's first post)
//   2. a real HN comment link → walks to its story, cursor lands on the comment
//   3. a broken HN link       → Not Found state
//   4. an external link       → opens in the browser

const REAL_STORY_LINK = "https://news.ycombinator.com/item?id=1"
const REAL_COMMENT_LINK = "https://news.ycombinator.com/item?id=15"
const BROKEN_LINK = "https://news.ycombinator.com/item?id=999999999999"
const FAKE_ID = 999_999_001

export const HnTransportLinkDemo = Layer.sync(HnTransport, () => {
  const live = makeLiveGetJson()
  let hookedStoryId: number | null = null

  const getJson: HnTransportShape["getJson"] = (path) => {
    // hook the first story of the first feed the app loads
    if (/^(top|new|best|ask|show|job)stories\.json$/.test(path)) {
      return live(path).pipe(
        Effect.tap((ids) =>
          Effect.sync(() => {
            if (Array.isArray(ids) && ids.length > 0) hookedStoryId ??= ids[0] as number
          }),
        ),
      )
    }
    if (hookedStoryId !== null && path === `item/${hookedStoryId}.json`) {
      return live(path).pipe(
        Effect.map((raw) => {
          const item = raw as { kids?: number[]; descendants?: number } | null
          if (!item) return raw
          return {
            ...item,
            kids: [FAKE_ID, ...(item.kids ?? [])],
            descendants: (item.descendants ?? 0) + 1,
          }
        }),
      )
    }
    if (path === `item/${FAKE_ID}.json`) {
      return Effect.succeed({
        id: FAKE_ID,
        type: "comment",
        by: "link-demo",
        time: Math.floor(Date.now() / 1000),
        parent: hookedStoryId ?? undefined,
        text:
          `[SIMULATED COMMENT] Press ⏎ on me to list my links: ` +
          `<a href="${REAL_STORY_LINK}">the first HN post ever</a>, ` +
          `<a href="${REAL_COMMENT_LINK}">a comment under it</a>, ` +
          `<a href="${BROKEN_LINK}">a broken HN link</a>, ` +
          `and <a href="https://example.com/">an external link</a>.`,
      })
    }
    return live(path)
  }

  return { getJson }
})
