use crate::color::Rgba;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeName {
    Dark,
    Light,
}

pub struct Theme {
    pub body: Rgba,
    pub strip: Rgba,
    pub border: Rgba,

    pub brand_tile_bg: Rgba,
    pub brand_tile_fg: Rgba,
    pub brand_text: Rgba,
    pub brand_subtle: Rgba,

    pub tab_active_bg: Rgba,
    pub tab_active_fg: Rgba,
    pub tab_inactive_fg: Rgba,

    pub status_hint: Rgba,

    pub text: Rgba,
    pub text_body: Rgba,
    pub text_muted: Rgba,
    pub text_dim: Rgba,
    pub text_visited: Rgba,

    pub accent: Rgba,
    pub strip_accent: Rgba,
    pub link: Rgba,

    pub row_highlight: Rgba,

    // context menu: panel, and the label colour on the highlighted item
    pub menu_bg: Rgba,
    pub menu_hover_fg: Rgba,

    pub scroll_track: Rgba,
    pub scroll_thumb: Rgba,

    pub comment_depth: [Rgba; 6],
}

const fn h(s: &str) -> Rgba {
    Rgba::hex(s)
}

pub static DARK: Theme = Theme {
    body: h("#0a0a0acc"),
    strip: h("#0e0e0e"),
    border: h("#2a2a2a"),

    brand_tile_bg: h("#ff6600"),
    brand_tile_fg: h("#000000"),
    brand_text: h("#ff6600"),
    brand_subtle: h("#888888"),

    tab_active_bg: h("#ff6600"),
    tab_active_fg: h("#000000"),
    tab_inactive_fg: h("#cccccc"),

    status_hint: h("#888888"),

    text: h("#ffffff"),
    text_body: h("#dddddd"),
    text_muted: h("#aaaaaa"),
    text_dim: h("#666666"),
    text_visited: h("#666666"),

    accent: h("#ff6600"),
    strip_accent: h("#ff6600"),
    link: h("#4488ff"),

    row_highlight: h("#ffffff1a"),

    menu_bg: h("#1a1a1a"),
    menu_hover_fg: h("#000000"),

    scroll_track: h("#1a1a1a"),
    scroll_thumb: h("#555555"),

    comment_depth: [h("#ff6600"), h("#ffaa44"), h("#ffd58a"), h("#88ccff"), h("#aa88ff"), h("#ff88cc")],
};

pub static LIGHT: Theme = Theme {
    body: h("#f6f6ef"),
    strip: h("#ff6600"),
    border: h("#f6f6ef"),

    brand_tile_bg: h("#ffffff"),
    brand_tile_fg: h("#000000"),
    brand_text: h("#000000"),
    brand_subtle: h("#5a2200"),

    tab_active_bg: h("#ffffff"),
    tab_active_fg: h("#000000"),
    tab_inactive_fg: h("#000000"),

    status_hint: h("#000000"),

    text: h("#000000"),
    text_body: h("#000000"),
    text_muted: h("#000000"),
    text_dim: h("#000000"),
    text_visited: h("#828282"),

    accent: h("#ff6600"),
    strip_accent: h("#000000"),
    link: h("#4488ff"),

    row_highlight: h("#ffffff"),

    menu_bg: h("#f6f6ef"),
    menu_hover_fg: h("#ffffff"),

    scroll_track: h("#ffffff"),
    scroll_thumb: h("#828282"),

    comment_depth: [h("#ff6600"), h("#cc5200"), h("#993d00"), h("#0066cc"), h("#6633aa"), h("#aa3366")],
};

impl ThemeName {
    pub fn theme(self) -> &'static Theme {
        match self {
            ThemeName::Dark => &DARK,
            ThemeName::Light => &LIGHT,
        }
    }
    pub fn toggled(self) -> ThemeName {
        match self {
            ThemeName::Dark => ThemeName::Light,
            ThemeName::Light => ThemeName::Dark,
        }
    }
}
