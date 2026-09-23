import { useEffect, useMemo, useRef, useState } from "react"
import { Effect, Fiber } from "effect"
import type { ScrollBoxRenderable } from "@opentui/core"
import { useKeyboard, useRenderer } from "@opentui/react"
import { Header } from "./components/Header"
import { StatusBar } from "./components/StatusBar"
import { StoryListView } from "./views/StoryListView"
import { StoryDetailView } from "./views/StoryDetailView"
import { MessageView } from "./views/MessageView"
import { useStoryIds } from "./hooks/useStoryIds"
import { useItems } from "./hooks/useItems"
import { flattenTree, useCommentTree } from "./hooks/useCommentTree"
import { useSaved } from "./hooks/useSaved"
import { useHistory } from "./hooks/useHistory"
import { useUpdateCheck } from "./hooks/useUpdateCheck"
import { ALL_CATEGORIES, FEED_CATEGORIES } from "./api/types"
import type { Category, FeedCategory, Item } from "./api/types"
import { openUrl } from "./utils/openUrl"
import { extractLinks, parseHnItemLink, type HnItemRef, type Link } from "./utils/format"
import { resolveStory } from "./api/hn"
import type { HnError, HnItemGone } from "./api/hn"
import { AppRuntime } from "./runtime"
import { hnErrorMessage } from "./utils/errors"
import { startThemeWipe } from "./utils/themeWipe"
import { LinksPopup } from "./components/LinksPopup"
import { HelpOverlay } from "./components/HelpOverlay"
import { ContextMenu, type MenuItem } from "./components/ContextMenu"
import { ThemeContext, darkTheme, lightTheme } from "./theme"

const PAGE_SIZE = 30

type ResolveError = HnError | HnItemGone

type View =
  | { kind: "list" }
  | { kind: "detail"; story: Item }
  // a link that couldn't be opened — keeps the ref so `r` can retry
  | { kind: "resolveError"; ref: HnItemRef; error: ResolveError }

// A suspended detail view: enough state to resume it exactly where it was left
type DetailSnapshot = { story: Item; cursor: number; collapsed: Set<number> }

export function App() {
  const renderer = useRenderer()
  const [category, setCategory] = useState<Category>("top")
  const [refreshKey, setRefreshKey] = useState(0)
  const feedCategory: FeedCategory =
    category === "saved" || category === "history" ? "top" : category
  const { ids, loading: idsLoading, error: idsError } = useStoryIds(feedCategory, refreshKey)
  const visibleIds = useMemo(() => ids.slice(0, PAGE_SIZE), [ids])
  const { items: feedItems, loading: feedItemsLoading } = useItems(visibleIds)
  const { entries: savedEntries, idSet: savedIds, isSaved, toggle: toggleSave } = useSaved()
  const { entries: historyEntries, idSet: viewedIds, markViewed, clear: clearHistory } = useHistory()
  const updateAvailable = useUpdateCheck()

  const savedIdList = useMemo(() => savedEntries.map((e) => e.id), [savedEntries])
  const { items: savedItemsRaw, loading: savedLoading } = useItems(
    category === "saved" ? savedIdList : [],
  )
  const savedItems = useMemo(() => {
    if (category !== "saved") return [] as Item[]
    const byId = new Map(savedItemsRaw.map((i) => [i.id, i]))
    return savedIdList.map((id) => byId.get(id)).filter((x): x is Item => Boolean(x))
  }, [category, savedItemsRaw, savedIdList])

  // History can hold up to HISTORY_CAP ids (for the visited-dimming lookup); only the
  // most-recent page is fetched/browsable here. The full set still powers de-emphasis.
  const historyIdList = useMemo(
    () => historyEntries.slice(0, PAGE_SIZE).map((e) => e.id),
    [historyEntries],
  )
  const { items: historyItemsRaw, loading: historyLoading } = useItems(
    category === "history" ? historyIdList : [],
  )
  const historyItems = useMemo(() => {
    if (category !== "history") return [] as Item[]
    const byId = new Map(historyItemsRaw.map((i) => [i.id, i]))
    return historyIdList.map((id) => byId.get(id)).filter((x): x is Item => Boolean(x))
  }, [category, historyItemsRaw, historyIdList])

  const items =
    category === "saved" ? savedItems : category === "history" ? historyItems : feedItems
  const listLoading =
    category === "saved"
      ? savedLoading
      : category === "history"
        ? historyLoading
        : idsLoading || feedItemsLoading

  const [view, setView] = useState<View>({ kind: "list" })
  const [listCursor, setListCursor] = useState(0)
  const [detailCursor, setDetailCursor] = useState(0)
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set())
  const [theme, setTheme] = useState(darkTheme)
  const [popup, setPopup] = useState<{ links: Link[]; cursor: number } | null>(null)
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[]; cursor: number } | null>(
    null,
  )
  const [help, setHelp] = useState(false)
  const [stack, setStack] = useState<DetailSnapshot[]>([])
  const [resolving, setResolving] = useState(false)
  const [pendingFocus, setPendingFocus] = useState<number | null>(null)
  const [commentsRefresh, setCommentsRefresh] = useState(0)
  const resolveFiber = useRef<Fiber.RuntimeFiber<void, never> | null>(null)
  const lastG = useRef<number>(0)
  const listScrollRef = useRef<ScrollBoxRenderable | null>(null)
  const detailScrollRef = useRef<ScrollBoxRenderable | null>(null)

  const story = view.kind === "detail" ? view.story : null
  const { tree, loading: commentsLoading } = useCommentTree(story?.kids, 8, commentsRefresh)
  const flat = useMemo(() => flattenTree(tree, collapsed), [tree, collapsed])

  useEffect(() => {
    if (listCursor >= items.length) setListCursor(Math.max(0, items.length - 1))
  }, [items.length])

  useEffect(() => {
    // don't clamp while comments are (re)loading — a restored cursor from the
    // view stack must survive the brief flat=[] window during the reload
    if (!commentsLoading && detailCursor >= flat.length)
      setDetailCursor(Math.max(0, flat.length - 1))
  }, [flat.length, commentsLoading])

  // After following an internal link, land the cursor on the linked comment
  // (best effort — it may be deleted or deeper than the fetched tree)
  useEffect(() => {
    if (pendingFocus == null || commentsLoading) return
    const idx = flat.findIndex((f) => f.node.item.id === pendingFocus)
    if (idx >= 0) setDetailCursor(idx)
    setPendingFocus(null)
  }, [pendingFocus, commentsLoading, flat])

  // Speculative prefetch: while the links popup is open, resolve any internal
  // HN links in the background so ⏎ is instant. The item cache is the handoff —
  // no state needed here. Closing the popup interrupts all in-flight hops.
  const popupLinks = popup?.links ?? null
  useEffect(() => {
    if (!popupLinks) return
    const refs = popupLinks
      .map((l) => parseHnItemLink(l.url))
      .filter((r): r is HnItemRef => r !== null)
    if (refs.length === 0) return
    const fiber = AppRuntime.runFork(
      Effect.forEach(refs, (ref) => resolveStory(ref).pipe(Effect.ignore), {
        concurrency: 4,
      }),
    )
    return () => {
      AppRuntime.runFork(Fiber.interrupt(fiber))
    }
  }, [popupLinks])

  useEffect(() => {
    if (popup || menu || help) {
      detailScrollRef.current?.blur()
      listScrollRef.current?.blur()
    }
  }, [popup, menu, help])

  const openMenuForStory = (item: Item, x: number, y: number) => {
    const items: MenuItem[] = [
      {
        label: isSaved(item.id) ? "★ Unsave" : "☆ Save",
        action: () => toggleSave(item.id),
      },
    ]
    if (item.url) {
      items.push({
        label: "Open URL in browser",
        action: () => {
          markViewed(item.id)
          openUrl(item.url!)
        },
      })
    }
    items.push({ label: "Open comments", action: () => enterDetail(item) })
    setMenu({ x, y, items, cursor: 0 })
  }

  const switchCategory = (c: Category) => {
    setCategory(c)
    setListCursor(0)
  }

  const cycleCategory = (dir: 1 | -1) => {
    const idx = ALL_CATEGORIES.indexOf(category)
    const next = ALL_CATEGORIES[(idx + dir + ALL_CATEGORIES.length) % ALL_CATEGORIES.length]!
    switchCategory(next)
  }

  const cancelResolve = () => {
    if (resolveFiber.current) {
      AppRuntime.runFork(Fiber.interrupt(resolveFiber.current))
      resolveFiber.current = null
    }
    setResolving(false)
  }

  // Entering from the list starts fresh; entering from a detail view pushes
  // the current view (with cursor + collapsed state) onto the stack first.
  const enterDetail = (item: Item, focusId?: number) => {
    cancelResolve()
    markViewed(item.id)
    if (view.kind === "detail") {
      const snap = { story: view.story, cursor: detailCursor, collapsed }
      setStack((s) => [...s, snap])
    }
    setView({ kind: "detail", story: item })
    setDetailCursor(0)
    setCollapsed(new Set())
    setPendingFocus(focusId ?? null)
  }

  // esc/h goes back ONE level: resume the previous thread, or exit to the list
  const popView = () => {
    cancelResolve()
    setPendingFocus(null)
    const top = stack[stack.length - 1]
    if (!top) {
      setView({ kind: "list" })
      return
    }
    setStack((s) => s.slice(0, -1))
    setView({ kind: "detail", story: top.story })
    setDetailCursor(top.cursor)
    setCollapsed(top.collapsed)
  }

  // the Y tile / global shortcuts abandon the whole stack
  const goHome = () => {
    cancelResolve()
    setPendingFocus(null)
    setStack([])
    setView({ kind: "list" })
  }

  const showResolveError = (ref: HnItemRef, error: ResolveError) => {
    // retrying from an existing error view replaces it — only a detail
    // view being left behind needs a snapshot pushed
    if (view.kind === "detail") {
      const snap = { story: view.story, cursor: detailCursor, collapsed }
      setStack((s) => [...s, snap])
    }
    setView({ kind: "resolveError", ref, error })
  }

  const startResolve = (ref: HnItemRef) => {
    cancelResolve()
    setResolving(true)
    resolveFiber.current = AppRuntime.runFork(
      resolveStory(ref).pipe(
        Effect.match({
          onSuccess: (r) => {
            resolveFiber.current = null
            setResolving(false)
            enterDetail(r.story, r.focusId)
          },
          onFailure: (error) => {
            resolveFiber.current = null
            setResolving(false)
            showResolveError(ref, error)
          },
        }),
      ),
    )
  }

  const openLink = (link: Link) => {
    const ref = parseHnItemLink(link.url)
    if (ref) {
      setPopup(null)
      startResolve(ref)
    } else {
      openUrl(link.url)
    }
  }

  const openHnLink = (id: number) => openUrl(`https://news.ycombinator.com/item?id=${id}`)

  const openLinksFor = (id: number) => {
    const fc = flat.find((f) => f.node.item.id === id)
    if (!fc) return
    const links = extractLinks(fc.node.item.text)
    if (links.length > 0) setPopup({ links, cursor: 0 })
  }

  const toggleCollapse = (id: number) => {
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const pageSize = (kind: "list" | "detail") => {
    const sb = kind === "list" ? listScrollRef.current : detailScrollRef.current
    const rowApprox = kind === "list" ? 2 : 4
    return Math.max(1, Math.floor((sb?.viewport.height ?? 20) / rowApprox))
  }

  useKeyboard((ev) => {
    const name = ev.name
    if (name === "q" || (ev.ctrl && name === "c")) {
      renderer?.destroy()
      process.exit(0)
    }

    const isHelpKey = name === "?" || (name === "/" && ev.shift)

    if (help) {
      if (name === "escape" || name === "backspace" || isHelpKey) setHelp(false)
      return
    }

    if (menu) {
      const max = menu.items.length - 1
      if (name === "j" || name === "down") {
        setMenu((m) => (m ? { ...m, cursor: Math.min(max, m.cursor + 1) } : m))
      } else if (name === "k" || name === "up") {
        setMenu((m) => (m ? { ...m, cursor: Math.max(0, m.cursor - 1) } : m))
      } else if (name === "return" || name === "enter") {
        const item = menu.items[menu.cursor]
        if (item && !item.disabled) item.action()
        setMenu(null)
      } else if (name === "escape" || name === "backspace") {
        setMenu(null)
      }
      return
    }

    if (popup) {
      const max = popup.links.length - 1
      if (name === "j" || name === "down") {
        setPopup((p) => (p ? { ...p, cursor: Math.min(max, p.cursor + 1) } : p))
      } else if (name === "k" || name === "up") {
        setPopup((p) => (p ? { ...p, cursor: Math.max(0, p.cursor - 1) } : p))
      } else if (name === "g" && ev.shift) {
        setPopup((p) => (p ? { ...p, cursor: max } : p))
      } else if (name === "g") {
        setPopup((p) => (p ? { ...p, cursor: 0 } : p))
      } else if (name === "return" || name === "enter") {
        const link = popup.links[popup.cursor]
        if (link) openLink(link)
      } else if (name === "o") {
        const link = popup.links[popup.cursor]
        if (link) openUrl(link.url)
      } else if (name === "escape" || name === "backspace") {
        setPopup(null)
      }
      return
    }

    // esc while a link is resolving cancels it (aborts the fetch chain)
    if (resolving && (name === "escape" || name === "backspace")) {
      cancelResolve()
      return
    }

    if (isHelpKey) {
      setHelp(true)
      return
    }

    if (name === "t") {
      const next = theme.name === "dark" ? lightTheme : darkTheme
      // snapshot the old-theme frame BEFORE React re-renders in the new theme
      if (renderer) startThemeWipe(renderer, String(next.accent))
      setTheme(next)
      return
    }

    // Capital S enters saved view from anywhere
    if (name === "s" && ev.shift) {
      goHome()
      switchCategory("saved")
      return
    }

    // Capital H enters history view from anywhere
    if (name === "h" && ev.shift) {
      goHome()
      switchCategory("history")
      return
    }

    if (view.kind === "list") {
      const max = items.length - 1
      const pg = pageSize("list")
      if (name === "j" || name === "down") {
        setListCursor((c) => Math.min(max, c + 1))
      } else if (name === "k" || name === "up") {
        setListCursor((c) => Math.max(0, c - 1))
      } else if (name === "g" && ev.shift) {
        setListCursor(max)
      } else if (name === "g") {
        const now = Date.now()
        if (now - lastG.current < 500) setListCursor(0)
        lastG.current = now
      } else if ((ev.ctrl && name === "d") || name === "pagedown") {
        setListCursor((c) => Math.min(max, c + pg))
      } else if ((ev.ctrl && name === "u") || name === "pageup") {
        setListCursor((c) => Math.max(0, c - pg))
      } else if (name === "c" || name === "return" || name === "enter") {
        const cur = items[listCursor]
        if (cur) enterDetail(cur)
      } else if (name === "h" || name === "left") {
        cycleCategory(-1)
      } else if (name === "l" || name === "right") {
        cycleCategory(1)
      } else if (name === "tab") {
        cycleCategory(ev.shift ? -1 : 1)
      } else if (name === "o") {
        const cur = items[listCursor]
        if (cur?.url) {
          markViewed(cur.id)
          openUrl(cur.url)
        }
      } else if (name === "y") {
        const cur = items[listCursor]
        if (cur) {
          markViewed(cur.id)
          openHnLink(cur.id)
        }
      } else if (name === "s") {
        const cur = items[listCursor]
        if (cur) toggleSave(cur.id)
      } else if (name === "x" && category === "history") {
        clearHistory()
      } else if (/^[1-6]$/.test(name)) {
        const c = FEED_CATEGORIES[parseInt(name, 10) - 1]
        if (c) switchCategory(c.key)
      } else if (name === "r" && category !== "saved" && category !== "history") {
        setRefreshKey((k) => k + 1)
      }
    } else if (view.kind === "resolveError") {
      if (name === "h" || name === "left" || name === "backspace" || name === "escape") {
        popView()
      } else if (name === "r" && view.error._tag !== "HnItemGone") {
        // a gone post stays gone — only transient failures earn a retry
        startResolve(view.ref)
      }
    } else {
      const max = flat.length - 1
      const pg = pageSize("detail")
      if (name === "j" || name === "down") {
        setDetailCursor((c) => Math.min(max, c + 1))
      } else if (name === "k" || name === "up") {
        setDetailCursor((c) => Math.max(0, c - 1))
      } else if (name === "g" && ev.shift) {
        setDetailCursor(max)
      } else if (name === "g") {
        const now = Date.now()
        if (now - lastG.current < 500) setDetailCursor(0)
        lastG.current = now
      } else if ((ev.ctrl && name === "d") || name === "pagedown") {
        setDetailCursor((c) => Math.min(max, c + pg))
      } else if ((ev.ctrl && name === "u") || name === "pageup") {
        setDetailCursor((c) => Math.max(0, c - pg))
      } else if (name === "space") {
        const cur = flat[detailCursor]
        if (cur) toggleCollapse(cur.node.item.id)
      } else if (name === "return" || name === "enter") {
        const cur = flat[detailCursor]
        if (cur) openLinksFor(cur.node.item.id)
      } else if (name === "o") {
        if (view.story.url) openUrl(view.story.url)
      } else if (name === "y") {
        openHnLink(view.story.id)
      } else if (name === "s") {
        toggleSave(view.story.id)
      } else if (name === "r") {
        // retry comments only when they failed to load entirely
        if (!commentsLoading && flat.length === 0 && (view.story.descendants ?? 0) > 0) {
          setCommentsRefresh((k) => k + 1)
        }
      } else if (name === "h" || name === "left" || name === "backspace" || name === "escape") {
        popView()
      }
    }
  })

  const detailLoading = commentsLoading
  const statusLoading =
    (view.kind === "list" ? listLoading : view.kind === "detail" ? detailLoading : false) ||
    resolving

  return (
    <ThemeContext.Provider value={theme}>
      <box flexDirection="column" flexGrow={1}>
        <Header
          category={category}
          onSelect={switchCategory}
          onHome={() => {
            if (view.kind !== "list") {
              goHome()
            } else if (category === "saved" || category === "history") {
              // saved/history are local lists — nothing to refresh, just reset the cursor
              setListCursor(0)
            } else {
              setRefreshKey((k) => k + 1)
            }
          }}
          showTabs={view.kind === "list"}
          depth={stack.length}
        />
        <box flexGrow={1} flexDirection="column" backgroundColor={theme.body}>
          {view.kind === "list" && idsError && category !== "saved" && category !== "history" ? (
            <MessageView
              art="(×_×)"
              title="Couldn't load stories"
              subtitle={hnErrorMessage(idsError)}
              hint="r to retry"
            />
          ) : view.kind === "list" ? (
            <StoryListView
              key={category}
              ref={listScrollRef}
              items={items}
              cursor={listCursor}
              loading={listLoading}
              savedIds={savedIds}
              viewedIds={viewedIds}
              emptyMessage={
                category === "saved"
                  ? "No saved posts yet. Press 's' on a story."
                  : category === "history"
                    ? "No history yet. Posts you open will show up here."
                    : ids.length > 0
                      ? "Couldn't load stories — press r to retry."
                      : "No stories"
              }
              loadingMessage={
                category === "saved"
                  ? "Loading saved posts…"
                  : category === "history"
                    ? "Loading history…"
                    : "Loading stories…"
              }
              onSelect={setListCursor}
              onActivate={(idx) => {
                const cur = items[idx]
                if (cur) enterDetail(cur)
              }}
              onContextMenu={(idx, ev) => {
                const cur = items[idx]
                if (cur) openMenuForStory(cur, ev.x, ev.y)
              }}
            />
          ) : view.kind === "detail" ? (
            <StoryDetailView
              key={view.story.id}
              ref={detailScrollRef}
              story={view.story}
              flat={flat}
              cursor={detailCursor}
              collapsed={collapsed}
              loading={detailLoading}
              saved={isSaved(view.story.id)}
              emptyMessage={
                (view.story.descendants ?? 0) > 0
                  ? "Couldn't load comments — press r to retry."
                  : undefined
              }
              onSelectComment={setDetailCursor}
              onToggleComment={toggleCollapse}
              onOpenLinks={openLinksFor}
            />
          ) : view.error._tag === "HnItemGone" ? (
            <MessageView
              art={"¯\\_(ツ)_/¯"}
              title="Post not found"
              subtitle="This link points to a post that doesn't exist (or was deleted)."
              hint="esc to go back"
            />
          ) : (
            <MessageView
              art="(×_×)"
              title="Couldn't open that post"
              subtitle={hnErrorMessage(view.error)}
              hint="r to retry · esc to go back"
            />
          )}
        </box>
        <StatusBar
          view={view.kind === "detail" ? "detail" : "list"}
          category={category}
          loading={statusLoading}
          updateAvailable={updateAvailable}
          message={
            view.kind === "resolveError"
              ? view.error._tag === "HnItemGone"
                ? "h/esc go back · q quit"
                : "r retry · h/esc go back · q quit"
              : view.kind === "list" && idsError
                ? "r retry · q quit"
                : undefined
          }
        />
        {menu ? (
          <ContextMenu
            x={menu.x}
            y={menu.y}
            items={menu.items}
            cursor={menu.cursor}
            onSelect={(idx) => setMenu((m) => (m ? { ...m, cursor: idx } : m))}
            onActivate={(idx) => {
              const item = menu.items[idx]
              if (item && !item.disabled) item.action()
              setMenu(null)
            }}
            onClose={() => setMenu(null)}
          />
        ) : null}
        {popup ? (
          <LinksPopup
            links={popup.links}
            cursor={popup.cursor}
            onSelect={(idx) => setPopup((p) => (p ? { ...p, cursor: idx } : p))}
            onActivate={(idx) => {
              const link = popup.links[idx]
              if (link) openLink(link)
            }}
            onClose={() => setPopup(null)}
          />
        ) : null}
        {help ? (
          <HelpOverlay
            view={view.kind === "list" ? "list" : "detail"}
            onClose={() => setHelp(false)}
          />
        ) : null}
      </box>
    </ThemeContext.Provider>
  )
}
