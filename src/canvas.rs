// A cell grid with OpenTUI's compositing rules. Everything is drawn here
// first (with real alpha), then copied into ratatui's buffer, which handles
// diffing and writing escape codes to the terminal.
//
// ratatui's own Buffer stores opaque colors only, so translucent layers (the
// dimmed overlay backdrop, the row highlight, the 80% body) need this layer.

use crate::color::{blend, Rgba};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct Cell {
    pub sym: String,
    pub fg: Rgba,
    pub bg: Rgba,
    pub bold: bool,
    // right half of a wide glyph drawn in the cell to the left
    pub cont: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { sym: " ".into(), fg: Rgba::WHITE, bg: Rgba::TRANSPARENT, bold: false, cont: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w: w.max(0), h: h.max(0) }
    }
    pub fn intersect(self, o: Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Style of one run of text. `bg` transparent = draw over whatever is there.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    pub fg: Rgba,
    pub bg: Rgba,
    pub bold: bool,
}

impl Style {
    pub fn fg(fg: Rgba) -> Style {
        Style { fg, bg: Rgba::TRANSPARENT, bold: false }
    }
    pub fn bold(self) -> Style {
        Style { bold: true, ..self }
    }
    pub fn bold_if(self, b: bool) -> Style {
        Style { bold: b, ..self }
    }
    pub fn bg(self, bg: Rgba) -> Style {
        Style { bg, ..self }
    }
}

/// One grapheme with its display width and style: the unit of layout.
#[derive(Clone, Debug)]
pub struct G {
    pub s: String,
    pub w: usize,
    pub st: Style,
}

pub fn styled(text: &str, st: Style) -> Vec<G> {
    text.graphemes(true)
        .map(|g| {
            // tabs render as a single space cell; HN text basically never has them
            let s = if g == "\t" { " " } else { g };
            G { s: s.to_string(), w: if s == "\n" || s == "\r\n" { 0 } else { s.width() }, st }
        })
        .collect()
}

pub fn width_of(gs: &[G]) -> usize {
    gs.iter().map(|g| g.w).sum()
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub cells: Vec<Cell>,
    clip: Vec<Rect>,
}

impl Canvas {
    pub fn new(w: u16, h: u16) -> Canvas {
        let (w, h) = (w as i32, h as i32);
        Canvas { w, h, cells: vec![Cell::default(); (w * h) as usize], clip: vec![Rect::new(0, 0, w, h)] }
    }

    pub fn full(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    pub fn push_clip(&mut self, r: Rect) {
        let top = *self.clip.last().unwrap();
        self.clip.push(top.intersect(r));
    }

    pub fn pop_clip(&mut self) {
        if self.clip.len() > 1 {
            self.clip.pop();
        }
    }

    /// OpenTUI's setCellWithAlphaBlending: an opaque cell replaces what's
    /// there; a translucent one blends into it — and a translucent *space*
    /// keeps the glyph underneath (dimmed), which is how overlays let the
    /// screen show through.
    pub fn put(&mut self, x: i32, y: i32, sym: &str, w: usize, fg: Rgba, bg: Rgba, bold: bool) {
        if !self.clip.last().unwrap().contains(x, y) {
            return;
        }
        let idx = (y * self.w + x) as usize;
        let dest = self.cells[idx].clone();
        let cell = if bg.a < 1.0 || fg.a < 1.0 {
            let preserve = sym == " " && dest.sym != " " && !dest.sym.is_empty() && !dest.cont;
            let final_bg = blend(bg, dest.bg);
            if preserve {
                Cell { sym: dest.sym, fg: blend(bg, dest.fg), bg: final_bg, bold: dest.bold, cont: false }
            } else {
                let final_fg = if fg.a < 1.0 { blend(fg, dest.bg) } else { fg };
                Cell { sym: sym.to_string(), fg: final_fg, bg: final_bg, bold, cont: false }
            }
        } else {
            Cell { sym: sym.to_string(), fg, bg, bold, cont: false }
        };
        let bg_after = cell.bg;
        self.cells[idx] = cell;
        if w == 2 && x + 1 < self.w && self.clip.last().unwrap().contains(x + 1, y) {
            let c = &mut self.cells[idx + 1];
            *c = Cell { sym: String::new(), fg, bg: bg_after, bold, cont: true };
        }
    }

    pub fn fill(&mut self, r: Rect, bg: Rgba) {
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                self.put(x, y, " ", 1, Rgba::WHITE, bg, false);
            }
        }
    }

    /// Draw one line of styled graphemes starting at (x, y).
    pub fn text(&mut self, x: i32, y: i32, gs: &[G]) -> i32 {
        let mut cx = x;
        for g in gs {
            if g.w == 0 {
                continue;
            }
            self.put(cx, y, &g.s, g.w, g.st.fg, g.st.bg, g.st.bold);
            cx += g.w as i32;
        }
        cx
    }

    pub fn str(&mut self, x: i32, y: i32, s: &str, st: Style) -> i32 {
        self.text(x, y, &styled(s, st))
    }

    pub fn hline(&mut self, x: i32, y: i32, w: i32, ch: &str, fg: Rgba, bg: Rgba) {
        for i in 0..w {
            self.put(x + i, y, ch, 1, fg, bg, false);
        }
    }

    /// Bordered box, OpenTUI style: interior fill, frame drawn with the box
    /// background (each cell blended once), title two cells in from the
    /// left corner replacing that stretch of the top edge.
    pub fn frame(&mut self, r: Rect, rounded: bool, fg: Rgba, bg: Rgba, title: Option<&str>) {
        self.fill(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), bg);
        let (tl, tr, bl, br) = if rounded { ("╭", "╮", "╰", "╯") } else { ("┌", "┐", "└", "┘") };
        let (x1, y1) = (r.x + r.w - 1, r.y + r.h - 1);
        let title_gs = title.map(|t| styled(t, Style::fg(fg).bg(bg))).unwrap_or_default();
        let (t0, t1) = (r.x + 2, r.x + 2 + width_of(&title_gs) as i32);
        self.put(r.x, r.y, tl, 1, fg, bg, false);
        self.put(x1, r.y, tr, 1, fg, bg, false);
        self.put(r.x, y1, bl, 1, fg, bg, false);
        self.put(x1, y1, br, 1, fg, bg, false);
        for x in r.x + 1..x1 {
            if !(t0..t1).contains(&x) {
                self.put(x, r.y, "─", 1, fg, bg, false);
            }
            self.put(x, y1, "─", 1, fg, bg, false);
        }
        for y in r.y + 1..y1 {
            self.put(r.x, y, "│", 1, fg, bg, false);
            self.put(x1, y, "│", 1, fg, bg, false);
        }
        self.text(t0, r.y, &title_gs);
    }

    /// Copy into ratatui's buffer. Alpha is dropped here, exactly like
    /// OpenTUI's output stage: the terminal only ever sees the composite.
    pub fn flush_into(&self, buf: &mut Buffer) {
        let area = buf.area;
        for y in 0..self.h.min(area.height as i32) {
            for x in 0..self.w.min(area.width as i32) {
                let c = &self.cells[(y * self.w + x) as usize];
                let out = &mut buf[(x as u16, y as u16)];
                let (fr, fg, fb) = c.fg.to_u8();
                let (br, bg, bb) = c.bg.to_u8();
                out.reset();
                out.set_symbol(if c.cont || c.sym.is_empty() { " " } else { &c.sym });
                out.set_fg(Color::Rgb(fr, fg, fb));
                out.set_bg(Color::Rgb(br, bg, bb));
                if c.bold {
                    out.modifier.insert(Modifier::BOLD);
                }
            }
        }
    }
}
