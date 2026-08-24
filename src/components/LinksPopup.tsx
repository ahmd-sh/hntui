import { TextAttributes } from "@opentui/core"
import type { Link } from "../utils/format"
import { selectionColors, useTheme } from "../theme"

interface Props {
  links: Link[]
  cursor: number
  onSelect: (idx: number) => void
  onActivate: (idx: number) => void
  onClose: () => void
}

export function LinksPopup({ links, cursor, onSelect, onActivate, onClose }: Props) {
  const t = useTheme()

  return (
    <box
      position="absolute"
      top={0}
      left={0}
      width="100%"
      height="100%"
      alignItems="center"
      justifyContent="center"
      zIndex={100}
      backgroundColor="#00000088"
      onMouseDown={onClose}
    >
      <box
        flexDirection="column"
        width="70%"
        backgroundColor={t.body}
        border={true}
        borderStyle="rounded"
        borderColor={t.accent}
        paddingTop={1}
        paddingBottom={1}
        paddingLeft={2}
        paddingRight={2}
        title={` Links (${links.length}) `}
        onMouseDown={(ev) => ev.stopPropagation()}
      >
        {links.map((link, idx) => (
          <LinkRow
            key={`${idx}-${link.url}`}
            link={link}
            index={idx}
            selected={idx === cursor}
            isFirst={idx === 0}
            onSelect={() => onSelect(idx)}
            onActivate={() => onActivate(idx)}
          />
        ))}
        <box marginTop={1}>
          <text fg={t.statusHint} {...selectionColors(t)}>j/k move · ⏎ open · o browser · esc close</text>
        </box>
      </box>
    </box>
  )
}

interface RowProps {
  link: Link
  index: number
  selected: boolean
  isFirst: boolean
  onSelect: () => void
  onActivate: () => void
}

function LinkRow({ link, index, selected, isFirst, onSelect, onActivate }: RowProps) {
  const t = useTheme()
  let lastClick = 0
  const handleClick = () => {
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
      flexDirection="column"
      marginTop={isFirst ? 0 : 1}
      backgroundColor={selected ? t.rowHighlight : undefined}
      paddingLeft={1}
      paddingRight={1}
      onMouseDown={handleClick}
    >
      <text {...selectionColors(t)}>
        <span fg={selected ? t.accent : t.textDim}>{selected ? "▶ " : "  "}</span>
        <span fg={t.text} attributes={selected ? TextAttributes.BOLD : TextAttributes.NONE}>
          {`${index + 1}. ${link.text}`}
        </span>
      </text>
      <text fg={t.link} {...selectionColors(t)}>{`    ${link.url}`}</text>
    </box>
  )
}
