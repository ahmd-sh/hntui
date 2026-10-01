import { describe, expect, test } from "bun:test"
import type { FlatComment } from "./useCommentTree"
import { nextRootIndex, rootIndexOf } from "./useCommentTree"

let nextId = 0
const fc = (depth: number): FlatComment => ({
  node: { item: { id: ++nextId, type: "comment" }, children: [] },
  depth,
  hiddenChildren: 0,
})

// threads: [root, child, grandchild], [root], [root, child]
const flat = [fc(0), fc(1), fc(2), fc(0), fc(0), fc(1)]

describe("rootIndexOf", () => {
  test("nested comment resolves to its thread root", () => {
    expect(rootIndexOf(flat, 2)).toBe(0)
    expect(rootIndexOf(flat, 5)).toBe(4)
  })
  test("a root resolves to itself", () => {
    expect(rootIndexOf(flat, 3)).toBe(3)
  })
  test("empty list gives -1", () => {
    expect(rootIndexOf([], 0)).toBe(-1)
  })
})

describe("nextRootIndex", () => {
  test("jumps forward over nested comments", () => {
    expect(nextRootIndex(flat, 0, 1)).toBe(3)
    expect(nextRootIndex(flat, 1, 1)).toBe(3)
  })
  test("backward from nested lands on own root first", () => {
    expect(nextRootIndex(flat, 2, -1)).toBe(0)
    expect(nextRootIndex(flat, 5, -1)).toBe(4)
  })
  test("backward from a root goes to the previous root", () => {
    expect(nextRootIndex(flat, 4, -1)).toBe(3)
  })
  test("clamps at the ends with -1", () => {
    expect(nextRootIndex(flat, 4, 1)).toBe(-1)
    expect(nextRootIndex(flat, 0, -1)).toBe(-1)
  })
})
