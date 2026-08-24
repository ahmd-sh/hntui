import { TextAttributes } from "@opentui/core"
import { selectionColors, useTheme } from "../theme"

interface Binding {
  keys: string
  desc: string
}

interface Group {
  title: string
  bindings: Binding[]
}

const NAVIGATION: Binding[] = [
  { keys: "j k  ↑ ↓", desc: "move down / up" },
  { keys: "g g / G", desc: "jump to top / bottom" },
  { keys: "Ctrl+d / Ctrl+u", desc: "half-page down / up" },
]

const LIST_GROUPS: Group[] = [
  {
    title: "Navigation",
    bindings: [
      ...NAVIGATION,
      { keys: "h / l", desc: "previous / next category" },
      { keys: "Tab / ⇧Tab", desc: "cycle categories" },
      { keys: "1 – 6", desc: "jump to category" },
    ],
  },
  {
    title: "Actions",
    bindings: [
      { keys: "c / ⏎", desc: "open comments" },
      { keys: "o", desc: "open post link in browser" },
      { keys: "y", desc: "open HN page in browser" },
      { keys: "s", desc: "save / unsave" },
      { keys: "r", desc: "refresh feed" },
      { keys: "x", desc: "clear history (History view)" },
    ],
  },
]

const DETAIL_GROUPS: Group[] = [
  {
    title: "Navigation",
    bindings: NAVIGATION,
  },
  {
    title: "Actions",
    bindings: [
      { keys: "space", desc: "collapse / expand" },
      { keys: "⏎", desc: "comment links (HN posts open in-app)" },
      { keys: "o", desc: "open post link in browser" },
      { keys: "y", desc: "open HN page in browser" },
      { keys: "s", desc: "save / unsave" },
      { keys: "h / esc", desc: "go back" },
    ],
  },
]

const GENERAL: Group = {
  title: "General",
  bindings: [
    { keys: "S", desc: "saved posts" },
    { keys: "H", desc: "view history" },
    { keys: "t", desc: "toggle theme" },
    { keys: "?", desc: "toggle this help" },
    { keys: "q", desc: "quit" },
  ],
}

interface Props {
  view: "list" | "detail"
  onClose: () => void
}

export function HelpOverlay({ view, onClose }: Props) {
  const t = useTheme()
  const groups = [...(view === "list" ? LIST_GROUPS : DETAIL_GROUPS), GENERAL]
  const keyWidth = groups.reduce(
    (w, g) => g.bindings.reduce((m, b) => Math.max(m, b.keys.length), w),
    0,
  )

  return (
    <box
      position="absolute"
      top={0}
      left={0}
      width="100%"
      height="100%"
      alignItems="center"
      justifyContent="center"
      zIndex={150}
      backgroundColor="#00000088"
      onMouseDown={onClose}
    >
      <box
        flexDirection="column"
        width="60%"
        backgroundColor={t.body}
        border={true}
        borderStyle="rounded"
        borderColor={t.accent}
        paddingTop={1}
        paddingBottom={1}
        paddingLeft={2}
        paddingRight={2}
        title=" Keyboard shortcuts "
        onMouseDown={(ev) => ev.stopPropagation()}
      >
        {groups.map((group, gi) => (
          <box key={group.title} flexDirection="column" marginTop={gi === 0 ? 0 : 1}>
            <text fg={t.accent} {...selectionColors(t)} attributes={TextAttributes.BOLD}>
              {group.title}
            </text>
            {group.bindings.map((b, bi) => (
              <text key={bi} {...selectionColors(t)}>
                <span fg={t.text}>{`  ${b.keys.padEnd(keyWidth)}`}</span>
                <span fg={t.textMuted}>{`   ${b.desc}`}</span>
              </text>
            ))}
          </box>
        ))}
        <box marginTop={1}>
          <text fg={t.statusHint} {...selectionColors(t)}>
            esc / ? close
          </text>
        </box>
      </box>
    </box>
  )
}
