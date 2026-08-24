import { useEffect, useState } from "react"
import { Effect, Fiber } from "effect"
import { fetchItem } from "../api/hn"
import type { Item } from "../api/types"

export interface CommentNode {
  item: Item
  children: CommentNode[]
}

export interface FlatComment {
  node: CommentNode
  depth: number
  hiddenChildren: number
}

const loadTree = (
  id: number,
  depth: number,
  maxDepth: number,
): Effect.Effect<CommentNode | null> =>
  Effect.gen(function* () {
    // a comment that fails to load is simply omitted, like before
    const item = yield* fetchItem(id).pipe(Effect.orElseSucceed(() => null))
    if (!item || item.deleted || item.dead) return null
    const kidIds = item.kids ?? []
    let children: CommentNode[] = []
    if (depth < maxDepth && kidIds.length > 0) {
      const loaded = yield* Effect.forEach(
        kidIds,
        (kid) => loadTree(kid, depth + 1, maxDepth),
        { concurrency: "unbounded" },
      )
      children = loaded.filter((x): x is CommentNode => x !== null)
    }
    return { item, children }
  })

export function useCommentTree(rootIds: number[] | undefined, maxDepth = 8, refreshKey = 0) {
  const idsKey = rootIds && rootIds.length > 0 ? rootIds.join(",") : ""
  // refreshKey re-runs the load for the same story: failed fetches were
  // evicted from the item cache, so a retry actually refetches them
  const key = idsKey === "" ? "" : `${refreshKey}|${idsKey}`
  // The tree is tagged with the key it was loaded for, so switching stories
  // DERIVES empty+loading state on the very same render
  const [result, setResult] = useState<{ key: string; tree: CommentNode[] }>({
    key: "",
    tree: [],
  })

  useEffect(() => {
    if (key === "") return
    const ids = key.slice(key.indexOf("|") + 1).split(",").map(Number)
    const fiber = Effect.runFork(
      Effect.forEach(ids, (id) => loadTree(id, 0, maxDepth), {
        concurrency: "unbounded",
      }).pipe(
        Effect.andThen((nodes) =>
          Effect.sync(() => {
            setResult({ key, tree: nodes.filter((x): x is CommentNode => x !== null) })
          }),
        ),
      ),
    )
    return () => {
      // interrupts the ENTIRE tree of pending comment fetches at once
      Effect.runFork(Fiber.interrupt(fiber))
    }
  }, [key, maxDepth])

  const fresh = key !== "" && result.key === key
  return { tree: fresh ? result.tree : [], loading: key !== "" && !fresh }
}

export function flattenTree(tree: CommentNode[], collapsed: Set<number>): FlatComment[] {
  const out: FlatComment[] = []
  const walk = (nodes: CommentNode[], depth: number) => {
    for (const node of nodes) {
      const isCollapsed = collapsed.has(node.item.id)
      const hidden = isCollapsed ? countAll(node) : 0
      out.push({ node, depth, hiddenChildren: hidden })
      if (!isCollapsed) walk(node.children, depth + 1)
    }
  }
  walk(tree, 0)
  return out
}

function countAll(node: CommentNode): number {
  let n = node.children.length
  for (const c of node.children) n += countAll(c)
  return n
}
