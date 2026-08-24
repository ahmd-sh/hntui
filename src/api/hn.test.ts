import { describe, expect, test } from "bun:test"
import { Effect, Layer } from "effect"
import { HnApiLive, HnTransport, fetchItem, resolveStory } from "./hn"

// A transport made of canned JSON. The REAL HnApiLive layer — schema
// decoding, the item cache, resolveStory's parent walk — runs on top of it,
// hermetically. Before Layers this required monkeypatching globalThis.fetch.
const apiWith = (db: Record<string, unknown>, log?: string[]) =>
  HnApiLive.pipe(
    Layer.provide(
      Layer.succeed(HnTransport, {
        getJson: (path) =>
          Effect.sync(() => {
            log?.push(path)
            return db[path] ?? null
          }),
      }),
    ),
  )

const DB = {
  "item/1.json": { id: 1, type: "story", title: "Root story", kids: [2, 4] },
  "item/2.json": { id: 2, type: "comment", parent: 1, text: "child" },
  "item/3.json": { id: 3, type: "comment", parent: 2, text: "grandchild" },
  "item/4.json": { id: 4, type: "comment", parent: 1, deleted: true },
  "item/5.json": { id: 5, type: "story", title: "Deleted story", deleted: true },
  "item/9.json": { id: "not-a-number", type: "story" },
}

describe("resolveStory (hermetic, via test transport layer)", () => {
  test("a grandchild comment walks up to the root story and focuses itself", async () => {
    const r = await Effect.runPromise(Effect.provide(resolveStory({ id: 3 }), apiWith(DB)))
    expect(r.story.id).toBe(1)
    expect(r.focusId).toBe(3)
  })

  test("an explicit #anchor wins over the walked-from comment", async () => {
    const r = await Effect.runPromise(
      Effect.provide(resolveStory({ id: 1, anchorId: 2 }), apiWith(DB)),
    )
    expect(r.story.id).toBe(1)
    expect(r.focusId).toBe(2)
  })

  test("an unknown id fails with HnItemGone", async () => {
    const e = await Effect.runPromise(
      Effect.provide(Effect.flip(resolveStory({ id: 404 })), apiWith(DB)),
    )
    expect(e._tag).toBe("HnItemGone")
  })

  test("a deleted story fails with HnItemGone", async () => {
    const e = await Effect.runPromise(
      Effect.provide(Effect.flip(resolveStory({ id: 5 })), apiWith(DB)),
    )
    expect(e._tag).toBe("HnItemGone")
  })

  test("a deleted comment still walks through to its living root", async () => {
    const r = await Effect.runPromise(Effect.provide(resolveStory({ id: 4 }), apiWith(DB)))
    expect(r.story.id).toBe(1)
    expect(r.focusId).toBe(4)
  })

  test("a payload that violates the schema fails with HnDecodeError", async () => {
    const e = await Effect.runPromise(
      Effect.provide(Effect.flip(fetchItem(9)), apiWith(DB)),
    )
    expect(e._tag).toBe("HnDecodeError")
  })
})

describe("item cache (hermetic)", () => {
  test("repeated fetches of one id hit the transport once", async () => {
    const log: string[] = []
    const program = Effect.gen(function* () {
      yield* fetchItem(1)
      yield* fetchItem(1)
      yield* fetchItem(1)
    })
    await Effect.runPromise(Effect.provide(program, apiWith(DB, log)))
    expect(log.filter((p) => p === "item/1.json")).toHaveLength(1)
  })
})
