import { useEffect, useMemo, useRef, useState } from "react"
import type { ScrollBoxRenderable } from "@opentui/core"
import { useKeyboard, useRenderer } from "@opentui/react"
import { Header } from "./components/Header"
import { StatusBar } from "./components/StatusBar"
import { StoryListView } from "./views/StoryListView"
import { StoryDetailView } from "./views/StoryDetailView"
import { useStoryIds } from "./hooks/useStoryIds"
import { useItems } from "./hooks/useItems"
import { flattenTree, useCommentTree } from "./hooks/useCommentTree"
import { useSaved } from "./hooks/useSaved"
import { useHistory } from "./hooks/useHistory"
import { ALL_CATEGORIES, FEED_CATEGORIES } from "./api/types"
import type { Category, FeedCategory, Item } from "./api/types"
import { openUrl } from "./utils/openUrl"
import { extractLinks, type Link } from "./utils/format"
import { LinksPopup } from "./components/LinksPopup"
import { HelpOverlay } from "./components/HelpOverlay"
import { ContextMenu, type MenuItem } from "./components/ContextMenu"
import { ThemeContext, darkTheme, lightTheme } from "./theme"

const PAGE_SIZE = 30

type View = { kind: "list" } | { kind: "detail"; story: Item }

export function App() {
  const renderer = useRenderer()
  const [category, setCategory] = useState<Category>("top")
  const [refreshKey, setRefreshKey] = useState(0)
  const feedCategory: FeedCategory =
    category === "saved" || category === "history" ? "top" : category
  const { ids, loading: idsLoading } = useStoryIds(feedCategory, refreshKey)
  const visibleIds = useMemo(() => ids.slice(0, PAGE_SIZE), [ids])
  const { items: feedItems, loading: feedItemsLoading } = useItems(visibleIds)
  const { entries: savedEntries, idSet: savedIds, isSaved, toggle: toggleSave } = useSaved()
  const { entries: historyEntries, idSet: viewedIds, markViewed, clear: clearHistory } = useHistory()

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
  const lastG = useRef<number>(0)
  const listScrollRef = useRef<ScrollBoxRenderable | null>(null)
  const detailScrollRef = useRef<ScrollBoxRenderable | null>(null)

  const story = view.kind === "detail" ? view.story : null
  const { tree, loading: commentsLoading } = useCommentTree(story?.kids)
  const flat = useMemo(() => flattenTree(tree, collapsed), [tree, collapsed])

  useEffect(() => {
    if (listCursor >= items.length) setListCursor(Math.max(0, items.length - 1))
  }, [items.length])

  useEffect(() => {
    if (detailCursor >= flat.length) setDetailCursor(Math.max(0, flat.length - 1))
  }, [flat.length])

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

  const enterDetail = (item: Item) => {
    markViewed(item.id)
    setView({ kind: "detail", story: item })
    setDetailCursor(0)
    setCollapsed(new Set())
  }

  const exitDetail = () => setView({ kind: "list" })

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
      } else if (name === "o" || name === "return" || name === "enter") {
        const link = popup.links[popup.cursor]
        if (link) openUrl(link.url)
      } else if (name === "escape" || name === "backspace") {
        setPopup(null)
      }
      return
    }

    if (isHelpKey) {
      setHelp(true)
      return
    }

    if (name === "t") {
      setTheme((cur) => (cur.name === "dark" ? lightTheme : darkTheme))
      return
    }

    // Capital S enters saved view from anywhere
    if (name === "s" && ev.shift) {
      setView({ kind: "list" })
      switchCategory("saved")
      return
    }

    // Capital H enters history view from anywhere
    if (name === "h" && ev.shift) {
      setView({ kind: "list" })
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
      } else if (name === "h" || name === "left" || name === "backspace" || name === "escape") {
        exitDetail()
      }
    }
  })

  const detailLoading = commentsLoading
  const statusLoading = view.kind === "list" ? listLoading : detailLoading

  return (
    <ThemeContext.Provider value={theme}>
      <box flexDirection="column" flexGrow={1}>
        <Header
          category={category}
          onSelect={switchCategory}
          onHome={() => {
            if (view.kind === "detail") {
              exitDetail()
            } else if (category === "saved" || category === "history") {
              // saved/history are local lists — nothing to refresh, just reset the cursor
              setListCursor(0)
            } else {
              setRefreshKey((k) => k + 1)
            }
          }}
          showTabs={view.kind === "list"}
        />
        <box flexGrow={1} flexDirection="column" backgroundColor={theme.body}>
          {view.kind === "list" ? (
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
          ) : (
            <StoryDetailView
              key={view.story.id}
              ref={detailScrollRef}
              story={view.story}
              flat={flat}
              cursor={detailCursor}
              collapsed={collapsed}
              loading={detailLoading}
              saved={isSaved(view.story.id)}
              onSelectComment={setDetailCursor}
              onToggleComment={toggleCollapse}
              onOpenLinks={openLinksFor}
            />
          )}
        </box>
        <StatusBar view={view.kind} category={category} loading={statusLoading} />
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
              if (link) openUrl(link.url)
            }}
            onClose={() => setPopup(null)}
          />
        ) : null}
        {help ? <HelpOverlay view={view.kind} onClose={() => setHelp(false)} /> : null}
      </box>
    </ThemeContext.Provider>
  )
}
