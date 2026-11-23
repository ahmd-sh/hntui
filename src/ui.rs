// Rendering: an immediate-mode port of the React component tree. Every
// frame is drawn from scratch into a Canvas.
//
// This is where the port costs real work. OpenTUI gave the TS version
// flexbox (Yoga) and a scroll container that measures wrapped children;
// here layout is arithmetic: measure the wrapped lines, stack the blocks,
// and clip to the viewport. The numbers (paddings, gaps, margins) mirror the
// TSX props one-for-one so the screens line up cell for cell.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::api::{error_message, Item, ResolveError};
use crate::app::{App, Category, Hit, Overlay, Pane, Scroll, View};
use crate::canvas::{styled, width_of, Canvas, Rect, Style, G};
use crate::color::Rgba;
use crate::format::{hostname, html_to_text, relative_time};
use crate::spinner;
use crate::theme::{Theme, ThemeName};
use crate::wrap::wrap;

type Line = Vec<G>;
type Hits = Vec<(Rect, Hit)>;

fn spans(parts: &[(&str, Style)]) -> Vec<G> {
    parts.iter().flat_map(|(s, st)| styled(s, *st)).collect()
}

// Yoga rounds layout edges to whole cells
fn round_edges(start: f32, size: f32) -> (i32, i32) {
    let a = start.round() as i32;
    let b = (start + size).round() as i32;
    (a, b - a)
}

fn draw_lines(c: &mut Canvas, x: i32, y: i32, lines: &[Line]) {
    for (i, l) in lines.iter().enumerate() {
        c.text(x, y + i as i32, l);
    }
}

fn loader(c: &mut Canvas, clock: &mut spinner::Clock, x: i32, y: i32, now: Instant) {
    let f = clock.tick(now);
    for (i, (glyph, color)) in spinner::frame(f).iter().enumerate() {
        c.put(x + i as i32, y, glyph, 1, *color, Rgba::TRANSPARENT, false);
    }
}

pub fn render(app: &mut App, c: &mut Canvas) {
    let t = app.theme.theme();
    let now = Instant::now();
    let (w, h) = (c.w, c.h);
    let list = matches!(app.view, View::List);
    let mut hits = std::mem::take(&mut app.hits);
    hits.clear();

    let hh = header(app, c, t, list, &mut hits);
    let body = Rect::new(0, hh, w, h - hh - 2);
    c.fill(body, t.body);

    let items = app.items();
    match &app.view {
        View::List => {
            if let (Some(err), Category::Feed(_)) = (&app.ids_error, app.category) {
                let sub = error_message(err);
                message(c, t, body, "(×_×)", "Couldn't load stories", Some(&sub), Some("r to retry"));
            } else {
                list_view(app, c, t, body, &items, now, &mut hits);
            }
        }
        View::Detail { story } => {
            let story = story.clone();
            detail_view(app, c, t, body, &story, now, &mut hits);
        }
        View::ResolveError { error, .. } => match error {
            ResolveError::Gone => message(
                c,
                t,
                body,
                "¯\\_(ツ)_/¯",
                "Post not found",
                Some("This link points to a post that doesn't exist (or was deleted)."),
                Some("esc to go back"),
            ),
            ResolveError::Hn(e) => {
                let sub = error_message(e);
                message(c, t, body, "(×_×)", "Couldn't open that post", Some(&sub), Some("r to retry · esc to go back"))
            }
        },
    }

    status_bar(app, c, t, now);

    if app.popup.is_some() {
        links_popup(app, c, t, &mut hits);
    }
    if app.help {
        help_overlay(c, t, list, &mut hits);
    }
    if app.menu.is_some() {
        context_menu(app, c, t, &mut hits);
    }
    if app.notes.is_some() {
        notes_overlay(app, c, t, &mut hits);
    }
    app.hits = hits;

    app.status_spinner.end_frame();
    app.list_spinner.end_frame();
    app.comments_spinner.end_frame();
}

// ---- chrome ---------------------------------------------------------------

fn header(app: &App, c: &mut Canvas, t: &Theme, show_tabs: bool, hits: &mut Hits) -> i32 {
    let w = c.w;
    let hh = if show_tabs { 5 } else { 3 };
    c.fill(Rect::new(0, 0, w, hh), t.strip);

    // brand row: tile · gap 2 · name · gap 2 · subtitle
    let y = 1;
    let x = c.str(1, y, " Y ", Style::fg(t.brand_tile_fg).bg(t.brand_tile_bg).bold());
    hits.push((Rect::new(1, y, x - 1, 1), Hit::Home));
    let x = c.str(x + 2, y, "HackerNews", Style::fg(t.brand_text).bold());
    c.str(x + 2, y, "· TUI", Style::fg(t.brand_subtle));
    if app.depth() > 0 {
        let s = format!("↩ {}", app.depth());
        let sw = width_of(&styled(&s, Style::fg(t.brand_subtle))) as i32;
        c.str(w - 1 - sw, y, &s, Style::fg(t.brand_subtle));
    }

    if show_tabs {
        let y = 3;
        let mut tab = |c: &mut Canvas, x: i32, label: &str, cat: Category| -> i32 {
            let st = if app.category == cat {
                Style::fg(t.tab_active_fg).bg(t.tab_active_bg)
            } else {
                Style::fg(t.tab_inactive_fg)
            };
            let end = c.str(x, y, label, st);
            hits.push((Rect::new(x, y, end - x, 1), Hit::Tab(cat)));
            end
        };
        let mut x = 1;
        for (i, f) in crate::api::FeedCategory::ALL.iter().enumerate() {
            let label = format!(" {} {} ", i + 1, f.label());
            x = tab(c, x, &label, Category::Feed(*f)) + 1;
        }
        let saved = " [S]aved ";
        let hist = " [H]istory ";
        let sx = w - 1 - saved.len() as i32;
        let hx = sx - 1 - hist.len() as i32;
        tab(c, hx, hist, Category::History);
        tab(c, sx, saved, Category::Saved);
    }
    c.hline(0, hh - 1, w, "─", t.border, t.strip);
    hh
}

fn status_bar(app: &mut App, c: &mut Canvas, t: &Theme, now: Instant) {
    let (w, h) = (c.w, c.h);
    c.fill(Rect::new(0, h - 2, w, 2), t.strip);
    c.hline(0, h - 2, w, "─", t.border, t.strip);
    let y = h - 1;

    let list = matches!(app.view, View::List);
    let loading = match &app.view {
        View::List => app.list_loading(),
        View::Detail { .. } => app.comments_loading(),
        View::ResolveError { .. } => false,
    } || app.resolving;

    let message = match &app.view {
        View::ResolveError { error: ResolveError::Gone, .. } => Some("h/esc go back · q quit"),
        View::ResolveError { .. } => Some("r retry · h/esc go back · q quit"),
        View::List if app.ids_error.is_some() => Some("r retry · q quit"),
        _ => None,
    };
    let hints = if list {
        if app.category == Category::History {
            "j/k move · c/⏎ open · x clear history · q quit"
        } else {
            "j/k move · c/⏎ open · s save · q quit"
        }
    } else {
        "j/k move · space collapse · ⏎ links · h/esc back"
    };

    let mut x = 1;
    if loading {
        loader(c, &mut app.status_spinner, x, y, now);
        x += 8 + 1;
    }
    let room = (w - 1 - "? help".len() as i32 - 1 - x).max(0) as usize;
    match app.current_notice() {
        Some(n) => {
            let fg = if n.warn { t.accent } else { t.status_hint };
            let mut gs = styled(&n.text, Style::fg(fg));
            if width_of(&gs) > room {
                // cut to fit, ending in an ellipsis
                let mut used = 0;
                gs.retain(|g| {
                    used += g.w;
                    used < room
                });
                gs.extend(styled("…", Style::fg(fg)));
            }
            c.text(x, y, &gs);
        }
        None => {
            c.str(x, y, message.unwrap_or(hints), Style::fg(t.status_hint));
        }
    }

    let help = "? help";
    let hx = w - 1 - help.len() as i32;
    c.str(hx, y, help, Style::fg(t.status_hint));
    if let Some(v) = &app.update_available {
        let s = format!("v{v} · hntui update");
        let sw = width_of(&styled(&s, Style::fg(t.text_dim))) as i32;
        c.str(hx - 1 - sw, y, &s, Style::fg(t.text_dim));
    }
}

// Where Yoga puts a child centred in `free` cells. Yoga rounds positions,
// except for nodes with a measure function (text), which it floors.
fn centre(free: i32, text: bool) -> i32 {
    let v = free as f32 / 2.0;
    if text { v.floor() as i32 } else { v.round() as i32 }
}

// A centered column of lines with gap 1 (MessageView, empty states). A row
// drawn in a fully transparent colour stands in for a non-text node (the
// loader), which gets box rounding instead of text flooring.
fn centered_column(c: &mut Canvas, area: Rect, rows: &[Vec<G>]) -> Vec<(i32, i32)> {
    let n = rows.len() as i32;
    let total = n + (n - 1).max(0);
    let mut out = vec![];
    for (i, r) in rows.iter().enumerate() {
        let text = r.iter().any(|g| g.st.fg.a > 0.0);
        let rw = width_of(r) as i32;
        let x = area.x + centre(area.w - rw, text);
        let y = area.y + centre(area.h - total, text) + i as i32 * 2;
        c.text(x, y, r);
        out.push((x, y));
    }
    out
}

fn message(c: &mut Canvas, t: &Theme, area: Rect, art: &str, title: &str, sub: Option<&str>, hint: Option<&str>) {
    let mut rows = vec![styled(art, Style::fg(t.accent)), styled(title, Style::fg(t.text).bold())];
    rows.extend(sub.map(|s| styled(s, Style::fg(t.status_hint))));
    rows.extend(hint.map(|s| styled(s, Style::fg(t.status_hint))));
    centered_column(c, area, &rows);
}

// ---- scrolling ------------------------------------------------------------

fn scrollbar(c: &mut Canvas, t: &Theme, s: &Scroll, x: i32, y: i32, h: i32) {
    c.fill(Rect::new(x, y, 1, h), t.scroll_track);
    let (start, size) = s.thumb(h);
    let end = start + size;
    for ry in (start / 2).max(0)..=((end + 1) / 2 - 1).min(h - 1) {
        let (cs, ce) = (ry * 2, ry * 2 + 2);
        let cov = end.min(ce) - start.max(cs);
        let ch = if cov >= 2 {
            "█"
        } else if cov > 0 {
            if start.max(cs) == cs {
                "▀"
            } else {
                "▄"
            }
        } else {
            " "
        };
        c.put(x, y + ry, ch, 1, t.scroll_thumb, t.scroll_track, false);
    }
}

/// Lay out content inside a scroll container the way OpenTUI does. Text
/// wraps at the width it had when it was first laid out (the full area
/// width; the scrollbar isn't showing yet then), but its *height* is
/// measured at the live width, one column narrower once the scrollbar
/// shows. So a line that exactly fills the full width can reserve an extra
/// empty row. `measure(draw_w, live_w)` returns (blocks, total height).
/// Returns the blocks and the live width.
fn scroll_layout<T>(scroll: &mut Scroll, area: Rect, mut measure: impl FnMut(i32, i32) -> (T, i32)) -> (T, i32) {
    let (full, full_h) = measure(area.w, area.w);
    let (blocks, h, live) = if full_h > area.h {
        let live = area.w - 1;
        let (blocks, h) = measure(area.w, live);
        (blocks, h, live)
    } else {
        (full, full_h, area.w)
    };
    if scroll.needs_settle(area.w, area.h) {
        scroll.settle(area.w, area.h, h);
    }
    scroll.layout(h);
    (blocks, live)
}

/// Wrapped lines drawn at one width, laid out at the height they'd need at
/// another (see `scroll_layout`). Drawing stops at `rows`.
struct Para {
    lines: Arc<Vec<Line>>,
    rows: i32,
}

impl Para {
    fn new(gs: &[G], draw_w: i32, live_w: i32) -> Para {
        let lines = wrap(gs, draw_w.max(1) as usize);
        let rows = if live_w == draw_w { lines.len() } else { wrap(gs, live_w.max(1) as usize).len() } as i32;
        Para { lines: Arc::new(lines), rows }
    }

    fn draw(&self, c: &mut Canvas, x: i32, y: i32) {
        draw_lines(c, x, y, &self.lines[..self.lines.len().min(self.rows.max(0) as usize)]);
    }
}

/// Wrapped comment bodies, keyed by (item, width). Rewrapping a few hundred
/// comments on every frame was the main cost of scrolling a big thread.
#[derive(Default)]
pub struct WrapCache {
    theme: Option<ThemeName>,
    bodies: HashMap<(u64, i32), Arc<Vec<Line>>>,
}

impl WrapCache {
    fn body(&mut self, theme: ThemeName, item: &Item, width: i32) -> Arc<Vec<Line>> {
        if self.theme != Some(theme) || self.bodies.len() > 20_000 {
            self.bodies.clear();
            self.theme = Some(theme);
        }
        let t = theme.theme();
        self.bodies
            .entry((item.id, width.max(1)))
            .or_insert_with(|| {
                // no text, no lines (wrap would return one empty line)
                let text = html_to_text(item.text.as_deref());
                Arc::new(if text.is_empty() { vec![] } else { wrap(&styled(&text, Style::fg(t.text_body)), width.max(1) as usize) })
            })
            .clone()
    }

    fn para(&mut self, theme: ThemeName, item: &Item, draw_w: i32, live_w: i32) -> Para {
        let lines = self.body(theme, item, draw_w);
        let rows = if live_w == draw_w { lines.len() } else { self.body(theme, item, live_w).len() } as i32;
        Para { lines, rows }
    }
}

// ---- story list -----------------------------------------------------------

struct Row {
    y: i32,
    lines: Vec<Line>,
    h: i32,
}

fn list_view(app: &mut App, c: &mut Canvas, t: &Theme, body: Rect, items: &[Arc<Item>], now: Instant, hits: &mut Hits) {
    if items.is_empty() {
        app.list_scroll.reset();
        let loading = app.list_loading();
        let msg = match app.category {
            Category::Saved if loading => "Loading saved posts…",
            Category::History if loading => "Loading history…",
            _ if loading => "Loading stories…",
            Category::Saved => "No saved posts yet. Press 's' on a story.",
            Category::History => "No history yet. Posts you open will show up here.",
            _ if !app.ids.is_empty() => "Couldn't load stories — press r to retry.",
            _ => "No stories",
        };
        if loading {
            // loader + gap + text, centered as a column
            let placeholder = styled("        ", Style::fg(Rgba::TRANSPARENT));
            let pos = centered_column(c, body, &[placeholder, styled(msg, Style::fg(t.status_hint))]);
            loader(c, &mut app.list_spinner, pos[0].0, pos[0].1, now);
        } else {
            centered_column(c, body, &[styled(msg, Style::fg(t.status_hint))]);
        }
        return;
    }

    let cursor = app.list_cursor;
    let (rows, lw) = scroll_layout(&mut app.list_scroll, body, |draw_w, live_w| {
        let mut y = 0;
        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let (saved, visited) = (app.saved_ids.contains(&it.id), app.viewed_ids.contains(&it.id));
                let row = |w: i32| story_row(t, i, it, i == cursor, saved, visited, (w - 2).max(1) as usize);
                let lines = row(draw_w);
                let h = if live_w == draw_w { lines.len() } else { row(live_w).len() } as i32;
                let r = Row { y, lines, h };
                y += h;
                r
            })
            .collect();
        (rows, y)
    });

    let s = &mut app.list_scroll;
    if s.pending_into_view {
        if let Some(r) = rows.get(cursor) {
            s.into_view(r.y, r.y + r.h);
        }
        s.pending_into_view = false;
    }

    let viewport = Rect::new(body.x, body.y, lw, body.h);
    hits.push((body, Hit::ScrollArea(Pane::List)));
    c.push_clip(viewport);
    for (i, r) in rows.iter().enumerate() {
        let y = body.y + r.y - s.top;
        if y + r.h <= body.y || y >= body.y + body.h {
            continue;
        }
        let row = Rect::new(0, y, lw, r.h);
        if i == cursor {
            c.fill(row, t.row_highlight);
        }
        draw_lines(c, 1, y, &r.lines[..r.lines.len().min(r.h as usize)]);
        hits.push((row.intersect(viewport), Hit::Row(i)));
    }
    c.pop_clip();
    if s.visible() {
        scrollbar(c, t, s, body.x + body.w - 1, body.y, body.h);
        hits.push((Rect::new(body.x + body.w - 1, body.y, 1, body.h), Hit::Scrollbar(Pane::List)));
    }
}

fn story_row(t: &Theme, idx: usize, it: &Item, selected: bool, saved: bool, visited: bool, width: usize) -> Vec<Line> {
    let host = hostname(it.url.as_deref());
    let title_fg = if visited { t.text_visited } else if selected { t.text } else { t.text_body };
    let rank_fg = if visited { t.text_visited } else if selected { t.accent } else { t.text_dim };
    let vote_fg = if visited { t.text_visited } else { t.accent };
    let dim = if visited { t.text_visited } else { t.text_dim };
    let muted = if visited { t.text_visited } else { t.text_muted };

    let rank = format!("{:>3}. ", idx + 1);
    let host_s = format!(" ({host})");
    let mut l1: Vec<(&str, Style)> = vec![(&rank, Style::fg(rank_fg))];
    if saved {
        l1.push(("★ ", Style::fg(t.accent)));
    }
    l1.push((it.title.as_deref().unwrap_or("(untitled)"), Style::fg(title_fg).bold_if(selected)));
    if !host.is_empty() {
        l1.push((&host_s, Style::fg(dim)));
    }

    let n = it.descendants.unwrap_or(0);
    let score = format!("▲ {}", it.score.unwrap_or(0));
    let age = format!("  {}  | ", relative_time(it.time));
    let comments = format!("{n} comment{}", if n == 1 { "" } else { "s" });
    let l2 = [
        ("     ", Style::fg(t.text_dim)),
        (score.as_str(), Style::fg(vote_fg)),
        ("  by ", Style::fg(dim)),
        (it.by.as_deref().unwrap_or("?"), Style::fg(muted)),
        (age.as_str(), Style::fg(dim)),
        (comments.as_str(), Style::fg(muted)),
    ];
    let mut lines = wrap(&spans(&l1), width);
    lines.extend(wrap(&spans(&l2), width));
    lines
}

// ---- story detail ---------------------------------------------------------

enum Block {
    Text { y: i32, para: Para },
    Loading { y: i32 },
    Comment { y: i32, x: i32, w: i32, head: Para, body: Option<Para>, idx: usize },
}

fn detail_view(app: &mut App, c: &mut Canvas, t: &Theme, body: Rect, story: &Item, now: Instant, hits: &mut Hits) {
    let w = c.w;
    let tw = (w - 2).max(1) as usize;

    // header strip: padding 1 all round, border below
    let saved = app.is_saved(story.id);
    let title = format!("{}{}", if saved { "★ " } else { "" }, story.title.as_deref().unwrap_or("(untitled)"));
    let mut lines = wrap(&styled(&title, Style::fg(t.text).bold()), tw);
    let host = hostname(story.url.as_deref());
    let score = format!("▲ {}", story.score.unwrap_or(0));
    let age = format!("  {}  | ", relative_time(story.time));
    let ncom = format!("{} comments", story.descendants.unwrap_or(0));
    let host_s = format!("  · {host}");
    let mut meta = vec![
        (score.as_str(), Style::fg(t.strip_accent)),
        (" by ", Style::fg(t.text_dim)),
        (story.by.as_deref().unwrap_or("?"), Style::fg(t.text_muted)),
        (age.as_str(), Style::fg(t.text_dim)),
        (ncom.as_str(), Style::fg(t.text_muted)),
    ];
    if !host.is_empty() {
        meta.push((host_s.as_str(), Style::fg(t.text_dim)));
    }
    lines.extend(wrap(&spans(&meta), tw));
    if let Some(u) = &story.url {
        let url = wrap(&styled(u, Style::fg(t.link)), tw);
        let url_y = body.y + 1 + lines.len() as i32;
        hits.push((Rect::new(1, url_y, tw as i32, url.len() as i32), Hit::StoryUrl));
        lines.extend(url);
    }
    let hh = lines.len() as i32 + 3;
    c.fill(Rect::new(0, body.y, w, hh), t.strip);
    draw_lines(c, 1, body.y + 1, &lines);
    c.hline(0, body.y + hh - 1, w, "─", t.border, t.strip);

    let area = Rect::new(0, body.y + hh, w, body.h - hh);
    let loading = app.comments_loading();
    let cursor = app.detail_cursor;
    let empty_msg = if story.descendants.unwrap_or(0) > 0 {
        "Couldn't load comments — press r to retry."
    } else {
        "No comments yet."
    };
    let theme = app.theme;
    let flat = &app.flat;
    let collapsed = &app.collapsed;
    let cache = &mut app.wrap_cache;

    let (blocks, live_w) = scroll_layout(&mut app.detail_scroll, area, |draw_w, live_w| {
        let mut blocks = vec![];
        let mut y = 0;
        // content boxes have 1 column of padding each side
        let (inner, inner_live) = (draw_w - 2, live_w - 2);
        if story.text.as_deref().is_some_and(|s| !s.is_empty()) {
            let para = cache.para(theme, story, inner, inner_live);
            let n = para.rows;
            if !para.lines.is_empty() {
                blocks.push(Block::Text { y: y + 1, para });
                y += 1 + n;
            }
        }
        y += 1; // comments box paddingTop
        if loading {
            blocks.push(Block::Loading { y });
            y += 1;
        } else if flat.is_empty() {
            let para = Para::new(&styled(empty_msg, Style::fg(t.status_hint)), inner, inner_live);
            let n = para.rows;
            blocks.push(Block::Text { y, para });
            y += n;
        } else {
            for (idx, fc) in flat.iter().enumerate() {
                y += 1; // marginTop
                let indent = fc.depth as i32 * 2;
                let (bw, bw_live) = (inner - indent, inner_live - indent);
                let is_collapsed = collapsed.contains(&fc.node.item.id);
                let head = Para::new(&comment_head(t, fc, idx == cursor, is_collapsed), bw, bw_live);
                let has_body = !is_collapsed && fc.node.item.text.as_deref().is_some_and(|s| !s.is_empty());
                let body = has_body.then(|| cache.para(theme, &fc.node.item, bw, bw_live)).filter(|p| !p.lines.is_empty());
                let n = head.rows + body.as_ref().map_or(0, |b| b.rows);
                blocks.push(Block::Comment { y, x: 1 + indent, w: bw.max(1), head, body, idx });
                y += n;
            }
        }
        (blocks, y)
    });

    let s = &mut app.detail_scroll;
    if s.pending_into_view && !flat.is_empty() {
        let target = blocks.iter().find_map(|b| match b {
            Block::Comment { y, head, body, idx, .. } if *idx == cursor => {
                Some((*y, *y + head.rows + body.as_ref().map_or(0, |b| b.rows)))
            }
            _ => None,
        });
        if let Some((a, b)) = target {
            s.into_view(a, b);
        }
        s.pending_into_view = false;
    }

    let top = s.top;
    let viewport = Rect::new(area.x, area.y, live_w, area.h);
    hits.push((area, Hit::ScrollArea(Pane::Detail)));
    c.push_clip(viewport);
    for b in &blocks {
        match b {
            Block::Text { y, para } => para.draw(c, 1, area.y + y - top),
            Block::Loading { y } => {
                let yy = area.y + y - top;
                loader(c, &mut app.comments_spinner, 1, yy, now);
                c.str(1 + 8 + 1, yy, "Loading comments…", Style::fg(t.status_hint));
            }
            Block::Comment { y, x, w, head, body, idx } => {
                let yy = area.y + y - top;
                let n = head.rows + body.as_ref().map_or(0, |b| b.rows);
                if yy + n <= area.y || yy >= area.y + area.h {
                    continue;
                }
                // the comment's box follows the live viewport width
                let bw = (*w).min(live_w - 1 - *x);
                if *idx == cursor {
                    c.fill(Rect::new(*x, yy, bw, n), t.row_highlight);
                }
                head.draw(c, *x, yy);
                hits.push((Rect::new(*x, yy, bw, head.rows).intersect(viewport), Hit::CommentHead(*idx)));
                if let Some(b) = body {
                    b.draw(c, *x, yy + head.rows);
                    hits.push((Rect::new(*x, yy + head.rows, bw, b.rows).intersect(viewport), Hit::CommentBody(*idx)));
                }
            }
        }
    }
    c.pop_clip();
    if s.visible() {
        scrollbar(c, t, s, area.x + area.w - 1, area.y, area.h);
        hits.push((Rect::new(area.x + area.w - 1, area.y, 1, area.h), Hit::Scrollbar(Pane::Detail)));
    }
}

// "▶ author · age · [+] N replies"
fn comment_head(t: &Theme, fc: &crate::tree::Flat, selected: bool, collapsed: bool) -> Vec<G> {
    let item = &fc.node.item;
    let bar = t.comment_depth[fc.depth % t.comment_depth.len()];
    let age = format!(" · {}", relative_time(item.time));
    let hidden = fc.hidden_children;
    let fold = format!(
        " · {} {} repl{}",
        if collapsed { "[+]" } else { "[-]" },
        hidden,
        if hidden == 1 { "y" } else { "ies" }
    );
    let mut head = vec![
        (if selected { "▶ " } else { "│ " }, Style::fg(bar)),
        (item.by.as_deref().unwrap_or("?"), Style::fg(t.accent)),
        (age.as_str(), Style::fg(t.text_dim)),
    ];
    if hidden > 0 || collapsed {
        head.push((fold.as_str(), Style::fg(t.text_muted)));
    }
    spans(&head)
}

// ---- overlays -------------------------------------------------------------

const BACKDROP: Rgba = Rgba::hex("#00000088");

/// Backdrop + centered rounded box at `pct` of the width; returns the inner
/// content rect (inside border and 1/2 padding). A box taller than the
/// screen is cut to fit (OpenTUI squashed its lines on top of each other);
/// callers clip to the returned rect.
fn modal(c: &mut Canvas, t: &Theme, bg: Rgba, pct: f32, content_h: i32, title: &str, hits: &mut Hits, which: Overlay) -> Rect {
    c.fill(c.full(), BACKDROP);
    let bw_f = c.w as f32 * pct;
    let (x, bw) = round_edges((c.w as f32 - bw_f) / 2.0, bw_f);
    let bh = (content_h + 2 + 2).min(c.h);
    let content_h = bh - 4;
    let y = ((c.h - bh) as f32 / 2.0).round() as i32;
    let panel = Rect::new(x, y, bw, bh);
    c.frame(panel, true, t.accent, bg, Some(title));
    hits.push((c.full(), Hit::Backdrop(which)));
    hits.push((panel, Hit::Panel));
    Rect::new(x + 3, y + 2, bw - 6, content_h)
}

fn links_popup(app: &App, c: &mut Canvas, t: &Theme, hits: &mut Hits) {
    let p = app.popup.as_ref().unwrap();
    let bw = round_edges((c.w as f32 - c.w as f32 * 0.7) / 2.0, c.w as f32 * 0.7).1;
    let inner = (bw - 6).max(1);
    let rw = (inner - 2).max(1) as usize;
    let rows: Vec<Vec<Line>> = p
        .links
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let sel = i == p.cursor;
            let label = format!("{}. {}", i + 1, l.text);
            let mut lines = wrap(
                &spans(&[
                    (if sel { "▶ " } else { "  " }, Style::fg(if sel { t.accent } else { t.text_dim })),
                    (&label, Style::fg(t.text).bold_if(sel)),
                ]),
                rw,
            );
            lines.extend(wrap(&styled(&format!("    {}", l.url), Style::fg(t.link)), rw));
            lines
        })
        .collect();
    let content_h: i32 = rows.iter().map(|r| r.len() as i32).sum::<i32>() + (rows.len() as i32 - 1).max(0) + 2;
    let area = modal(c, t, t.body, 0.7, content_h, &format!(" Links ({}) ", p.links.len()), hits, Overlay::Links);
    // when the list doesn't fit, scroll it so the selected link shows
    let mut starts = vec![];
    let mut y = 0;
    for (i, r) in rows.iter().enumerate() {
        y += if i > 0 { 1 } else { 0 };
        starts.push(y);
        y += r.len() as i32;
    }
    let sel = (starts[p.cursor], starts[p.cursor] + rows[p.cursor].len() as i32);
    let off = (sel.1 - area.h).max(0).min(sel.0);
    c.push_clip(area);
    for (i, r) in rows.iter().enumerate() {
        let y = area.y + starts[i] - off;
        let row = Rect::new(area.x, y, area.w, r.len() as i32);
        if i == p.cursor {
            c.fill(row, t.row_highlight);
        }
        draw_lines(c, area.x + 1, y, r);
        hits.push((row.intersect(area), Hit::LinkRow(i)));
    }
    c.str(area.x, area.y + y + 1 - off, "j/k move · ⏎ open · o browser · esc close", Style::fg(t.status_hint));
    c.pop_clip();
}

const NAVIGATION: [(&str, &str); 3] =
    [("j k  ↑ ↓", "move down / up"), ("g g / G", "jump to top / bottom"), ("Ctrl+d / Ctrl+u", "half-page down / up")];

fn help_overlay(c: &mut Canvas, t: &Theme, list: bool, hits: &mut Hits) {
    let mut groups: Vec<(&str, Vec<(&str, &str)>)> = if list {
        let mut nav = NAVIGATION.to_vec();
        nav.extend([("h / l", "previous / next category"), ("Tab / ⇧Tab", "cycle categories"), ("1 – 6", "jump to category")]);
        vec![
            ("Navigation", nav),
            (
                "Actions",
                vec![
                    ("c / ⏎", "open comments"),
                    ("o", "open post link in browser"),
                    ("y", "open HN page in browser"),
                    ("Y", "copy link"),
                    ("s", "save / unsave"),
                    ("r", "refresh feed"),
                    ("x", "clear history (History view)"),
                ],
            ),
        ]
    } else {
        vec![
            ("Navigation", NAVIGATION.to_vec()),
            (
                "Actions",
                vec![
                    ("space", "collapse / expand"),
                    ("J / K", "next / previous thread"),
                    ("c", "fold / unfold current thread"),
                    ("⏎", "comment links (HN posts open in-app)"),
                    ("o", "open post link in browser"),
                    ("y", "open HN page in browser"),
                    ("Y", "copy comment / link"),
                    ("s", "save / unsave"),
                    ("h / esc", "go back"),
                ],
            ),
        ]
    };
    groups.push((
        "General",
        vec![("S", "saved posts"), ("H", "view history"), ("t", "toggle theme"), ("?", "toggle this help"), ("q", "quit")],
    ));
    // padEnd counts UTF-16 units; every key label here is BMP, so chars == units
    let key_w = groups.iter().flat_map(|g| g.1.iter()).map(|b| b.0.chars().count()).max().unwrap_or(0);

    let bw = round_edges((c.w as f32 - c.w as f32 * 0.6) / 2.0, c.w as f32 * 0.6).1;
    let inner = (bw - 6).max(1) as usize;
    let mut lines: Vec<Line> = vec![];
    for (gi, (title, binds)) in groups.iter().enumerate() {
        if gi > 0 {
            lines.push(vec![]);
        }
        lines.push(styled(title, Style::fg(t.accent).bold()));
        for (k, d) in binds {
            let pad = key_w - k.chars().count();
            let keys = format!("  {k}{}", " ".repeat(pad));
            let desc = format!("   {d}");
            lines.extend(wrap(&spans(&[(&keys, Style::fg(t.text)), (&desc, Style::fg(t.text_muted))]), inner));
        }
    }
    lines.push(vec![]);
    lines.push(styled("esc / ? close", Style::fg(t.status_hint)));
    let area = modal(c, t, t.body, 0.6, lines.len() as i32, " Keyboard shortcuts ", hits, Overlay::Help);
    c.push_clip(area);
    draw_lines(c, area.x, area.y, &lines);
    c.pop_clip();
}

/// Right-click menu on a story (ContextMenu.tsx): a plain bordered panel at
/// the pointer, nudged to stay on screen, over an invisible full-screen
/// catcher that closes it.
fn context_menu(app: &App, c: &mut Canvas, t: &Theme, hits: &mut Hits) {
    let m = app.menu.as_ref().unwrap();
    // label.length in JS counts UTF-16 units; the labels are all BMP
    let longest = m.items.iter().map(|i| i.label.chars().count() as i32).max().unwrap_or(0);
    let mw = (longest + 4).min(40);
    let mh = m.items.len() as i32 + 2;
    let x = m.x.max(0).min(c.w - mw);
    let y = m.y.max(0).min(c.h - mh - 1);
    let panel = Rect::new(x, y, mw, mh);
    c.frame(panel, false, t.text_muted, t.menu_bg, None);
    hits.push((c.full(), Hit::Backdrop(Overlay::Menu)));
    hits.push((panel, Hit::Panel));
    for (i, item) in m.items.iter().enumerate() {
        let row = Rect::new(x + 1, y + 1 + i as i32, mw - 2, 1);
        let selected = i == m.cursor;
        if selected {
            c.fill(row, t.text_muted);
        }
        let fg = if selected { t.menu_hover_fg } else { t.text };
        c.str(row.x + 1, row.y, &item.label, Style::fg(fg));
        hits.push((row, Hit::MenuItem(i)));
    }
}

/// "What's new in hntui x.y.z": the release notes, once after an update.
/// Scrolls when they don't fit.
fn notes_overlay(app: &mut App, c: &mut Canvas, t: &Theme, hits: &mut Hits) {
    let bw = round_edges((c.w as f32 - c.w as f32 * 0.7) / 2.0, c.w as f32 * 0.7).1;
    let inner = (bw - 6).max(1);
    let nv = app.notes.as_mut().unwrap();
    let lines = crate::notes::render(&nv.sections, t, inner as usize);
    let title = match nv.sections.as_slice() {
        [(v, _)] => format!(" What's new in hntui {v} "),
        _ => " What's new in hntui ".to_string(),
    };
    // text, a blank line, the key hint. Opaque, unlike help: the screen
    // showing through behind paragraphs of prose makes them hard to read.
    let area = modal(c, t, t.body.with_alpha(1.0), 0.7, lines.len() as i32 + 2, &title, hits, Overlay::Notes);
    hits.push((area, Hit::ScrollArea(Pane::Notes)));
    let visible = (area.h - 2).max(1);
    nv.max = (lines.len() as i32 - visible).max(0);
    nv.scroll = nv.scroll.clamp(0, nv.max);
    c.push_clip(Rect::new(area.x, area.y, area.w, visible));
    draw_lines(c, area.x, area.y, &lines[nv.scroll as usize..]);
    c.pop_clip();
    let more = if nv.scroll < nv.max { "j/k scroll · " } else { "" };
    let hint = format!("{more}esc close");
    c.push_clip(area);
    c.str(area.x, area.y + area.h - 1, &hint, Style::fg(t.status_hint));
    c.pop_clip();
}
