// Simulation harness for internal HN-link navigation.
//
//   bun --preload ./scripts/hn-link-demo.ts src/index.tsx
//
// Intercepts fetch in-process and injects a synthetic comment as the first
// comment of the first story on the feed. The comment carries four links:
//   1. a real HN story link  → opens in the TUI (item 1, pg's first HN post)
//   2. a real HN comment link → walks to its story, cursor lands on the comment
//   3. a broken HN link       → Not Found state
//   4. an external link       → opens in the browser
// No app code or real HN data is touched — delete this file when bored of it.

const REAL_STORY_LINK = "https://news.ycombinator.com/item?id=1"
const REAL_COMMENT_LINK = "https://news.ycombinator.com/item?id=15"
const BROKEN_LINK = "https://news.ycombinator.com/item?id=999999999999"
const FAKE_ID = 999_999_001

let hookedStoryId: number | null = null
const orig = globalThis.fetch

globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
  const url = String(input)

  // hook the first story of the first feed the app loads
  if (/\/v0\/(top|new|best|ask|show|job)stories\.json/.test(url)) {
    const res = await orig(input, init)
    const ids = (await res.json()) as number[] | null
    if (ids?.length) hookedStoryId ??= ids[0]!
    return Response.json(ids)
  }

  if (hookedStoryId !== null && url.endsWith(`/item/${hookedStoryId}.json`)) {
    const res = await orig(input, init)
    const item = (await res.json()) as { kids?: number[]; descendants?: number } | null
    if (item) {
      item.kids = [FAKE_ID, ...(item.kids ?? [])]
      item.descendants = (item.descendants ?? 0) + 1
    }
    return Response.json(item)
  }

  if (url.endsWith(`/item/${FAKE_ID}.json`)) {
    return Response.json({
      id: FAKE_ID,
      type: "comment",
      by: "link-demo",
      time: Math.floor(Date.now() / 1000),
      parent: hookedStoryId,
      text:
        `[SIMULATED COMMENT] Press ⏎ on me to list my links: ` +
        `<a href="${REAL_STORY_LINK}">the first HN post ever</a>, ` +
        `<a href="${REAL_COMMENT_LINK}">a comment under it</a>, ` +
        `<a href="${BROKEN_LINK}">a broken HN link</a>, ` +
        `and <a href="https://example.com/">an external link</a>.`,
    })
  }

  return orig(input, init)
}) as typeof fetch
