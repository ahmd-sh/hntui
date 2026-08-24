import type { MouseEvent } from "@opentui/core"
import { TextAttributes } from "@opentui/core"
import type { Item } from "../api/types"
import { hostname, relativeTime } from "../utils/format"
import { selectionColors, useTheme } from "../theme"

interface Props {
  rank: number
  item: Item
  selected: boolean
  saved?: boolean
  visited?: boolean
  onSelect: () => void
  onActivate: () => void
  onContextMenu?: (ev: MouseEvent) => void
}

export function StoryRow({ rank, item, selected, saved, visited, onSelect, onActivate, onContextMenu }: Props) {
  const t = useTheme()
  const host = hostname(item.url)
  const title = item.title ?? "(untitled)"
  const score = item.score ?? 0
  const author = item.by ?? "?"
  const age = relativeTime(item.time)
  const comments = item.descendants ?? 0
  const bg = selected ? t.rowHighlight : undefined
  const titleFg = visited ? t.textVisited : selected ? t.text : t.textBody
  const rankFg = visited ? t.textVisited : selected ? t.accent : t.textDim
  const voteFg = visited ? t.textVisited : t.accent
  const dimFg = visited ? t.textVisited : t.textDim
  const mutedFg = visited ? t.textVisited : t.textMuted

  let lastClick = 0
  const handleClick = (ev: MouseEvent) => {
    if (ev.button === 2) {
      onSelect()
      onContextMenu?.(ev)
      return
    }
    const now = Date.now()
    if (selected || now - lastClick < 400) {
      onActivate()
    } else {
      onSelect()
    }
    lastClick = now
  }

  return (
    <box
      id={`row-${rank}`}
      flexDirection="column"
      paddingLeft={1}
      paddingRight={1}
      backgroundColor={bg}
      onMouseDown={handleClick}
    >
      <text {...selectionColors(t)}>
        <span fg={rankFg}>{`${String(rank).padStart(3, " ")}. `}</span>
        {saved ? <span fg={t.accent}>{"★ "}</span> : null}
        <span fg={titleFg} attributes={selected ? TextAttributes.BOLD : TextAttributes.NONE}>
          {title}
        </span>
        {host ? <span fg={dimFg}>{` (${host})`}</span> : null}
      </text>
      <text {...selectionColors(t)}>
        <span fg={t.textDim}>     </span>
        <span fg={voteFg}>▲ {score}</span>
        <span fg={dimFg}>{`  by `}</span>
        <span fg={mutedFg}>{author}</span>
        <span fg={dimFg}>{`  ${age}  | `}</span>
        <span fg={mutedFg}>{`${comments} comment${comments === 1 ? "" : "s"}`}</span>
      </text>
    </box>
  )
}
