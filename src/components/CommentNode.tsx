import { useRef } from "react"
import type { CommentNode as CN } from "../hooks/useCommentTree"
import { htmlToText, relativeTime } from "../utils/format"
import { selectionColors, useTheme } from "../theme"

interface Props {
  node: CN
  depth: number
  collapsed: boolean
  selected: boolean
  hiddenChildren: number
  onSelect: () => void
  onToggle: () => void
  onOpenLinks: () => void
}

export function CommentNode({
  node,
  depth,
  collapsed,
  selected,
  hiddenChildren,
  onSelect,
  onToggle,
  onOpenLinks,
}: Props) {
  const t = useTheme()
  const author = node.item.by ?? "?"
  const age = relativeTime(node.item.time)
  const body = htmlToText(node.item.text)
  const accent = t.commentDepth[depth % t.commentDepth.length]
  const indent = depth * 2
  const bg = selected ? t.rowHighlight : undefined

  const lastClick = useRef(0)
  const checkDouble = () => {
    const now = Date.now()
    const isDouble = now - lastClick.current < 400
    lastClick.current = now
    return isDouble
  }

  const handleHeaderClick = () => {
    onSelect()
    onToggle()
  }

  const handleBodyClick = () => {
    if (checkDouble()) {
      onOpenLinks()
    } else {
      onSelect()
    }
  }

  return (
    <box
      id={`comment-${node.item.id}`}
      flexDirection="column"
      marginLeft={indent}
      marginTop={1}
      backgroundColor={bg}
    >
      <text {...selectionColors(t)} onMouseDown={handleHeaderClick}>
        <span fg={accent}>{selected ? "▶ " : "│ "}</span>
        <span fg={t.accent}>{author}</span>
        <span fg={t.textDim}>{` · ${age}`}</span>
        {hiddenChildren > 0 || collapsed ? (
          <span fg={t.textMuted}>
            {` · ${collapsed ? "[+]" : "[-]"} ${hiddenChildren} repl${hiddenChildren === 1 ? "y" : "ies"}`}
          </span>
        ) : null}
      </text>
      {!collapsed && body ? (
        <text fg={t.textBody} {...selectionColors(t)} wrapMode="word" onMouseDown={handleBodyClick}>
          {body}
        </text>
      ) : null}
    </box>
  )
}
