import { createContext, useContext } from "react"
import type { ColorInput } from "@opentui/core"

export interface Theme {
  name: "dark" | "light"

  body: ColorInput
  strip: ColorInput
  border: ColorInput

  brandTileBg: ColorInput
  brandTileFg: ColorInput
  brandText: ColorInput
  brandSubtle: ColorInput

  tabActiveBg: ColorInput
  tabActiveFg: ColorInput
  tabInactiveFg: ColorInput

  statusHint: ColorInput

  text: ColorInput
  textBody: ColorInput
  textMuted: ColorInput
  textDim: ColorInput
  textVisited: ColorInput

  accent: ColorInput
  stripAccent: ColorInput
  link: ColorInput

  rowHighlight: ColorInput
  menuBg: ColorInput

  selectionBg: ColorInput
  selectionFg: ColorInput

  scrollTrack: ColorInput
  scrollThumb: ColorInput

  commentDepth: readonly ColorInput[]
}

export const darkTheme: Theme = {
  name: "dark",
  body: "#0a0a0acc",
  strip: "#0e0e0e",
  border: "#2a2a2a",

  brandTileBg: "#ff6600",
  brandTileFg: "#000000",
  brandText: "#ff6600",
  brandSubtle: "#888888",

  tabActiveBg: "#ff6600",
  tabActiveFg: "#000000",
  tabInactiveFg: "#cccccc",

  statusHint: "#888888",

  text: "#ffffff",
  textBody: "#dddddd",
  textMuted: "#aaaaaa",
  textDim: "#666666",
  textVisited: "#666666",

  accent: "#ff6600",
  stripAccent: "#ff6600",
  link: "#4488ff",

  rowHighlight: "#ffffff1a",
  menuBg: "#1a1a1a",

  selectionBg: "#ffffff",
  selectionFg: "#000000",

  scrollTrack: "#1a1a1a",
  scrollThumb: "#555555",

  commentDepth: ["#ff6600", "#ffaa44", "#ffd58a", "#88ccff", "#aa88ff", "#ff88cc"],
}

export const lightTheme: Theme = {
  name: "light",
  body: "#f6f6ef",
  strip: "#ff6600",
  border: "#f6f6ef",

  brandTileBg: "#ffffff",
  brandTileFg: "#000000",
  brandText: "#000000",
  brandSubtle: "#5a2200",

  tabActiveBg: "#ffffff",
  tabActiveFg: "#000000",
  tabInactiveFg: "#000000",

  statusHint: "#000000",

  text: "#000000",
  textBody: "#000000",
  textMuted: "#000000",
  textDim: "#000000",
  textVisited: "#828282",

  accent: "#ff6600",
  stripAccent: "#000000",
  link: "#4488ff",

  rowHighlight: "#ffffff",
  menuBg: "#f6f6ef",

  selectionBg: "#000000",
  selectionFg: "#ffffff",

  scrollTrack: "#ffffff",
  scrollThumb: "#828282",

  commentDepth: ["#ff6600", "#cc5200", "#993d00", "#0066cc", "#6633aa", "#aa3366"],
}

export const ThemeContext = createContext<Theme>(darkTheme)

export function useTheme(): Theme {
  return useContext(ThemeContext)
}

/** Spread onto <text> so mouse selection inverts properly in both themes. */
export function selectionColors(t: Theme): { selectionBg: ColorInput; selectionFg: ColorInput } {
  return { selectionBg: t.selectionBg, selectionFg: t.selectionFg }
}
