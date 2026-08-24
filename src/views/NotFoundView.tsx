import { TextAttributes } from "@opentui/core"
import { selectionColors, useTheme } from "../theme"

export function NotFoundView() {
  const t = useTheme()
  return (
    <box flexGrow={1} flexDirection="column" alignItems="center" justifyContent="center" gap={1}>
      <text fg={t.accent} {...selectionColors(t)}>
        {"¯\\_(ツ)_/¯"}
      </text>
      <text fg={t.text} {...selectionColors(t)} attributes={TextAttributes.BOLD}>
        Post not found
      </text>
      <text fg={t.statusHint} {...selectionColors(t)}>
        This link points to a post that doesn't exist (or was deleted).
      </text>
      <text fg={t.statusHint} {...selectionColors(t)}>
        esc to go back
      </text>
    </box>
  )
}
