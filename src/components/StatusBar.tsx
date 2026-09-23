import { Loader } from "./Loader"
import type { Category } from "../api/types"
import { selectionColors, useTheme } from "../theme"

interface Props {
  view: "list" | "detail"
  category?: Category
  loading?: boolean
  message?: string
  updateAvailable?: string | null
}

export function StatusBar({ view, category, loading, message, updateAvailable }: Props) {
  const t = useTheme()
  const hints =
    view === "list"
      ? category === "history"
        ? "j/k move · c/⏎ open · x clear history · q quit"
        : "j/k move · c/⏎ open · s save · q quit"
      : "j/k move · space collapse · ⏎ links · h/esc back"
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
      <box flexGrow={1} />
      {updateAvailable ? (
        <text fg={t.textDim} {...selectionColors(t)}>{`v${updateAvailable} · hntui update`}</text>
      ) : null}
      <text fg={t.statusHint} {...selectionColors(t)}>? help</text>
    </box>
  )
}
