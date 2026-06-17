import { Loader } from "./Loader"
import { selectionColors, useTheme } from "../theme"

interface Props {
  view: "list" | "detail"
  loading?: boolean
  message?: string
}

export function StatusBar({ view, loading, message }: Props) {
  const t = useTheme()
  const hints =
    view === "list"
      ? "j/k move · c/⏎ open · s save · ? help · q quit"
      : "j/k move · space collapse · ⏎ links · ? help · h/esc back"
  return (
    <box
      flexDirection="row"
      flexShrink={0}
      height={2}
      paddingLeft={1}
      paddingRight={1}
      gap={1}
      alignItems="center"
      backgroundColor={t.strip}
      border={["top"]}
      borderStyle="single"
      borderColor={t.border}
    >
      {loading ? <Loader /> : null}
      <text fg={t.statusHint} {...selectionColors(t)}>{message ?? hints}</text>
    </box>
  )
}
