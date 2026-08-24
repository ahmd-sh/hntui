import { TextAttributes } from "@opentui/core"
import { selectionColors, useTheme } from "../theme"

interface Props {
  art: string
  title: string
  subtitle?: string
  hint?: string
}

// Full-body centered state: not-found, network failure, and friends.
export function MessageView({ art, title, subtitle, hint }: Props) {
  const t = useTheme()
  return (
    <box flexGrow={1} flexDirection="column" alignItems="center" justifyContent="center" gap={1}>
      <text fg={t.accent} {...selectionColors(t)}>
        {art}
      </text>
      <text fg={t.text} {...selectionColors(t)} attributes={TextAttributes.BOLD}>
        {title}
      </text>
      {subtitle ? (
        <text fg={t.statusHint} {...selectionColors(t)}>
          {subtitle}
        </text>
      ) : null}
      {hint ? (
        <text fg={t.statusHint} {...selectionColors(t)}>
          {hint}
        </text>
      ) : null}
    </box>
  )
}
