import { TextAttributes } from "@opentui/core"
import type { Category } from "../api/types"
import { FEED_CATEGORIES } from "../api/types"
import { selectionColors, useTheme } from "../theme"

interface Props {
  category: Category
  onSelect: (c: Category) => void
  onHome: () => void
  showTabs?: boolean
}

export function Header({ category, onSelect, onHome, showTabs = true }: Props) {
  const t = useTheme()
  return (
    <box
      flexDirection="column"
      flexShrink={0}
      height={showTabs ? 5 : 3}
      paddingTop={1}
      paddingLeft={1}
      paddingRight={1}
      backgroundColor={t.strip}
      border={["bottom"]}
      borderStyle="single"
      borderColor={t.border}
    >
      <box flexDirection="row" flexShrink={0} height={1} alignItems="center" gap={2}>
        <text
          bg={t.brandTileBg}
          fg={t.brandTileFg}
          {...selectionColors(t)}
          attributes={TextAttributes.BOLD}
          onMouseDown={onHome}
        >
          {" Y "}
        </text>
        <text fg={t.brandText} {...selectionColors(t)} attributes={TextAttributes.BOLD}>
          HackerNews
        </text>
        <text fg={t.brandSubtle} {...selectionColors(t)}>· TUI</text>
      </box>
      {showTabs ? (
        <box flexDirection="row" flexShrink={0} height={1} marginTop={1} gap={1}>
          {FEED_CATEGORIES.map((c, i) => {
            const active = c.key === category
            return (
              <box
                key={c.key}
                flexShrink={0}
                onMouseDown={() => onSelect(c.key)}
                backgroundColor={active ? t.tabActiveBg : undefined}
              >
                <text fg={active ? t.tabActiveFg : t.tabInactiveFg} {...selectionColors(t)}>
                  {` ${i + 1} ${c.label} `}
                </text>
              </box>
            )
          })}
          <box flexGrow={1} />
          <box
            flexShrink={0}
            onMouseDown={() => onSelect("history")}
            backgroundColor={category === "history" ? t.tabActiveBg : undefined}
          >
            <text fg={category === "history" ? t.tabActiveFg : t.tabInactiveFg} {...selectionColors(t)}>
              {" [H]istory "}
            </text>
          </box>
          <box
            flexShrink={0}
            onMouseDown={() => onSelect("saved")}
            backgroundColor={category === "saved" ? t.tabActiveBg : undefined}
          >
            <text fg={category === "saved" ? t.tabActiveFg : t.tabInactiveFg} {...selectionColors(t)}>
              {" [S]aved "}
            </text>
          </box>
        </box>
      ) : null}
    </box>
  )
}
