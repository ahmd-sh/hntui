import { useTerminalDimensions } from "@opentui/react"
import { selectionColors, useTheme } from "../theme"

export interface MenuItem {
  label: string
  action: () => void
  disabled?: boolean
}

interface Props {
  x: number
  y: number
  items: MenuItem[]
  cursor: number
  onSelect: (idx: number) => void
  onActivate: (idx: number) => void
  onClose: () => void
}

export function ContextMenu({ x, y, items, cursor, onSelect, onActivate, onClose }: Props) {
  const t = useTheme()
  const { width: termW, height: termH } = useTerminalDimensions()
  const longest = items.reduce((n, it) => Math.max(n, it.label.length), 0)
  const menuW = Math.min(longest + 4, 40)
  const menuH = items.length + 2
  const left = Math.min(Math.max(0, x), termW - menuW)
  const top = Math.min(Math.max(0, y), termH - menuH - 1)

  const hoverFg = t.name === "dark" ? "#000000" : "#ffffff"

  return (
    <box
      position="absolute"
      top={0}
      left={0}
      width="100%"
      height="100%"
      zIndex={200}
      onMouseDown={onClose}
    >
      <box
        position="absolute"
        top={top}
        left={left}
        width={menuW}
        flexDirection="column"
        backgroundColor={t.menuBg}
        border={true}
        borderStyle="single"
        borderColor={t.textMuted}
        onMouseDown={(ev) => ev.stopPropagation()}
      >
        {items.map((item, idx) => {
          const selected = idx === cursor
          const fg = item.disabled ? t.textDim : selected ? hoverFg : t.text
          const bg = selected && !item.disabled ? t.textMuted : undefined
          return (
            <box
              key={idx}
              backgroundColor={bg}
              paddingLeft={1}
              paddingRight={1}
              onMouseDown={() => {
                if (item.disabled) return
                onActivate(idx)
              }}
              onMouseOver={() => {
                if (!item.disabled) onSelect(idx)
              }}
            >
              <text fg={fg} {...selectionColors(t)}>{item.label}</text>
            </box>
          )
        })}
      </box>
    </box>
  )
}
