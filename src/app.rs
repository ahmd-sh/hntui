// App state and behavior — a port of App.tsx.
//
// React's model was "state + effects with dependency arrays". Here it's an
// explicit state struct, a `handle` for each message (key press or finished
// background task), and `sync()`, which plays the role of the effects: it
// compares what *should* be loading against what *is*, starting or aborting
// tasks. Every task result carries the key it was started for, so a stale
// result is dropped exactly like the TS hooks' cleanup did.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::api::{self, FeedCategory, Hn, HnError, Item, Resolved, ResolveError};
use crate::canvas::Rect;
use crate::clipboard;
use crate::color::Rgba;
use crate::format::{extract_links, html_to_text, now_millis, parse_hn_item_link, HnItemRef, Link};
use crate::spinner;
use crate::store::{self, HistoryEntry, SavedEntry, HISTORY_CAP};
use crate::theme::ThemeName;
use crate::tree::{self, Flat, Node};

pub const PAGE_SIZE: usize = 30;
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Feed(FeedCategory),
    History,
    Saved,
}

impl Category {
    const ALL: [Category; 8] = [
        Category::Feed(FeedCategory::Top),
        Category::Feed(FeedCategory::New),
        Category::Feed(FeedCategory::Best),
        Category::Feed(FeedCategory::Ask),
        Category::Feed(FeedCategory::Show),
        Category::Feed(FeedCategory::Job),
        Category::History,
        Category::Saved,
    ];
}

pub enum View {
    List,
    Detail { story: Arc<Item> },
    // a link that couldn't be opened — keeps the ref so `r` can retry
    ResolveError { r: HnItemRef, error: ResolveError },
}

// a suspended detail view: enough to resume it exactly where it was left
struct Snapshot {
    story: Arc<Item>,
    cursor: usize,
    collapsed: HashSet<u64>,
}

pub struct Popup {
    pub links: Vec<Link>,
    pub cursor: usize,
}

/// Scroll position plus the bits of OpenTUI's scroll container state that
/// affect how it looks (see `layout`).
#[derive(Default)]
pub struct Scroll {
    pub top: i32,
    pub content: i32,
    pub viewport: i32,
    slider_vp: Option<f32>,
    // whether the scrollbar's "viewport size" has settled (see `settle`)
    settled: bool,
    area_w: i32,
    // true when the container mounts before its content has been measured
    empty_mount: bool,
    pub pending_into_view: bool,
    // the scrollbar's own (fractional) position while it's being dragged;
    // scrolling snaps to whole rows but the thumb follows the pointer
    drag_value: Option<f32>,
}

impl Scroll {
    pub fn reset(&mut self) {
        *self = Scroll { pending_into_view: true, ..Default::default() };
    }

    pub fn reset_empty_mount(&mut self) {
        *self = Scroll { pending_into_view: true, empty_mount: true, ..Default::default() };
    }

    /// An OpenTUI quirk, reproduced on purpose because it's visible: the
    /// scrollbar's thumb size comes from a "viewport size" it only computes
    /// when the viewport's height changes. On the first layout it reads the
    /// viewport before the content (range 0), so the size clamps to 0.01 and
    /// the thumb is a half cell, unless the viewport changes once more. It
    /// does when the horizontal scrollbar row, shown by default for that
    /// first layout, survives Yoga's proportional flex-shrink: the row gets
    /// viewport/(content+1) cells and is there if that rounds to 1. Later
    /// changes (resizes) recompute it from the content at that time.
    /// `content` is the height laid out at the live width.
    pub fn needs_settle(&self, area_w: i32, viewport: i32) -> bool {
        !self.settled || self.area_w != area_w || self.viewport != viewport
    }

    pub fn settle(&mut self, area_w: i32, viewport: i32, content: i32) {
        let vp = viewport.max(1) as f32;
        let range = |c: i32| (c - viewport).max(0) as f32;
        let size = if self.settled {
            vp.min(range(content))
        } else {
            // the comments view mounts with only a loader in it
            let c = if self.empty_mount { 0 } else { content };
            let first = (viewport * c) as f32 / (c + 1) as f32;
            if (first + 0.5).floor() < viewport as f32 { vp.min(range(c)) } else { 0.0 }
        };
        self.slider_vp = Some(size.max(0.01));
        self.settled = true;
        self.area_w = area_w;
        self.viewport = viewport;
    }

    pub fn layout(&mut self, content: i32) {
        self.content = content;
        self.top = self.top.clamp(0, (content - self.viewport).max(0));
    }

    pub fn into_view(&mut self, start: i32, end: i32) {
        let (vs, ve) = (self.top, self.top + self.viewport);
        let size = end - start;
        let start_out = start < vs;
        let end_out = end > ve;
        let dy = if start_out && end_out {
            0
        } else if (start_out && size < self.viewport) || (end_out && size > self.viewport) {
            start - vs
        } else if (start_out && size > self.viewport) || (end_out && size < self.viewport) {
            end - ve
        } else {
            0
        };
        self.top = (self.top + dy).clamp(0, (self.content - self.viewport).max(0));
    }

    pub fn visible(&self) -> bool {
        self.content > self.viewport
    }

    fn range(&self) -> i32 {
        (self.content - self.viewport).max(0)
    }

    /// One wheel notch = one row (OpenTUI's default, linear acceleration).
    /// Returns whether anything moved.
    pub fn wheel(&mut self, dir: i32) -> bool {
        let top = (self.top + dir).clamp(0, self.range());
        let moved = top != self.top;
        self.top = top;
        moved
    }

    // a drag value only sticks while nothing else has moved the view
    fn slider_value(&self) -> f32 {
        match self.drag_value {
            Some(v) if v.round() as i32 == self.top => v,
            _ => self.top as f32,
        }
    }

    fn set_slider_value(&mut self, v: f32) {
        let v = v.clamp(0.0, self.range() as f32);
        self.drag_value = Some(v);
        self.top = v.round() as i32;
    }

    /// The thumb's rows on screen, relative to the track: (start, height).
    fn thumb_rows(&self, track: i32) -> (i32, i32) {
        let (vs, vsize) = self.thumb(track);
        let start = vs.div_euclid(2);
        let end = (vs + vsize + 1).div_euclid(2);
        (start, (end - start).max(1))
    }

    // Slider.calculateDragOffsetVirtual: where on the thumb it was grabbed
    fn drag_offset(&self, rel_y: i32, track: i32) -> f32 {
        let (vs, vsize) = self.thumb(track);
        let pos = (rel_y * 2).clamp(0, track * 2);
        (pos - vs).clamp(0, vsize) as f32
    }

    // Slider.updateValueFromMouseDirect: a click on the track jumps there
    fn slider_jump(&mut self, rel_y: i32, track: i32) {
        let ratio = if track == 0 { 0.0 } else { rel_y.clamp(0, track) as f32 / track as f32 };
        self.set_slider_value(ratio * self.range() as f32);
    }

    // Slider.updateValueFromMouseWithOffset: the thumb follows the pointer
    fn slider_drag(&mut self, rel_y: i32, track: i32, offset: f32) {
        let (_, vsize) = self.thumb(track);
        let max_start = (track * 2 - vsize).max(0) as f32;
        let desired = ((rel_y.clamp(0, track) * 2) as f32 - offset).clamp(0.0, max_start);
        let ratio = if max_start == 0.0 { 0.0 } else { desired / max_start };
        self.set_slider_value(ratio * self.range() as f32);
    }

    /// Thumb in half-cell units: (start, size) — OpenTUI's Slider math.
    pub fn thumb(&self, track: i32) -> (i32, i32) {
        let vtrack = track * 2;
        let range = (self.content - self.viewport).max(0) as f32;
        if range == 0.0 {
            return (0, vtrack);
        }
        let vp = self.slider_vp.unwrap_or(1.0).max(1.0); // Slider: max(1, viewPortSize)
        let size = ((vtrack as f32 * (vp / (range + vp))).floor() as i32).clamp(1, vtrack);
        let start = ((self.slider_value() / range) * (vtrack - size) as f32).round() as i32;
        (start, size)
    }
}

pub enum Msg {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize,
    Ids { key: (FeedCategory, u32), result: Result<Vec<u64>, HnError> },
    Items { slot: Slot, key: Vec<u64>, items: Vec<Arc<Item>> },
    Tree { key: String, tree: Vec<Arc<Node>> },
    Resolved { gen: u64, result: Result<Resolved, ResolveError> },
    Update(Option<String>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Feed,
    Saved,
    History,
}

pub enum Action {
    None,
    // nothing visible changed (e.g. the pointer moved over nothing): skip the redraw
    Unchanged,
    Quit,
    ThemeWipe(Rgba),
}

// ---- mouse ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pane {
    List,
    Detail,
    Notes,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlay {
    Help,
    Links,
    Menu,
    Notes,
}

/// Something clickable, recorded by the renderer with its rect. Later
/// entries sit on top; an event goes to the topmost entry under the pointer
/// and "bubbles" down the stack the way OpenTUI's events bubble to parents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Home,
    Tab(Category),
    Row(usize),
    CommentHead(usize),
    CommentBody(usize),
    StoryUrl,
    ScrollArea(Pane),
    Scrollbar(Pane),
    // a modal's dimmed surround (click = close) and its panel (swallows clicks)
    Backdrop(Overlay),
    Panel,
    LinkRow(usize),
    MenuItem(usize),
}

pub struct Menu {
    pub x: i32,
    pub y: i32,
    pub items: Vec<MenuItem>,
    pub cursor: usize,
    hover: Option<usize>,
}

pub struct MenuItem {
    pub label: String,
    action: MenuAction,
}

enum MenuAction {
    ToggleSave(u64),
    OpenUrl(u64, String),
    OpenComments(Arc<Item>),
    Copy(String),
}

struct Drag {
    pane: Pane,
    track: Rect,
    offset: f32,
}

const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// "What's new": release notes shown once after an update.
pub struct Notes {
    pub sections: Vec<(&'static str, &'static str)>,
    pub scroll: i32,
    // set by the renderer: how far the notes can scroll
    pub max: i32,
}

/// A short message in the status bar, in place of the key hints.
pub struct Notice {
    pub text: String,
    pub warn: bool,
    until: Instant,
}

#[derive(Default)]
struct ItemsSlot {
    key: Vec<u64>,
    items: Vec<Arc<Item>>,
    loading: bool,
    task: Option<JoinHandle<()>>,
}

pub struct App {
    tx: UnboundedSender<Msg>,
    hn: Arc<Hn>,

    pub category: Category,
    refresh_key: u32,
    ids_key: Option<(FeedCategory, u32)>,
    ids_task: Option<JoinHandle<()>>,
    pub ids: Vec<u64>,
    pub ids_loading: bool,
    pub ids_error: Option<HnError>,

    feed: ItemsSlot,
    saved_slot: ItemsSlot,
    history_slot: ItemsSlot,

    pub saved: Vec<SavedEntry>,
    pub history: Vec<HistoryEntry>,
    pub saved_ids: HashSet<u64>,
    pub viewed_ids: HashSet<u64>,
    pub update_available: Option<String>,
    pub notice: Option<Notice>,
    pub notes: Option<Notes>,

    pub view: View,
    pub list_cursor: usize,
    pub detail_cursor: usize,
    pub collapsed: HashSet<u64>,
    pub theme: ThemeName,
    pub popup: Option<Popup>,
    pub menu: Option<Menu>,
    pub help: bool,
    stack: Vec<Snapshot>,
    pub resolving: bool,
    resolve_gen: u64,
    resolve_task: Option<JoinHandle<()>>,
    resolve_ref: Option<HnItemRef>,
    pending_focus: Option<u64>,
    comments_refresh: u32,
    last_g: Option<Instant>,

    // comment tree, tagged with the key it was loaded for
    comments_key: String,
    tree_key: String,
    tree: Vec<Arc<Node>>,
    tree_task: Option<JoinHandle<()>>,
    pub flat: Vec<Flat>,

    prefetch_task: Option<JoinHandle<()>>,
    prefetch_for: usize,
    popup_gen: usize,

    // last-seen values for the "effects"
    prev_items_len: usize,
    prev_flat: (usize, bool),
    prev_list_cursor: usize,
    prev_detail_cursor: usize,

    pub list_scroll: Scroll,
    pub detail_scroll: Scroll,
    pub status_spinner: spinner::Clock,
    pub list_spinner: spinner::Clock,
    pub comments_spinner: spinner::Clock,

    // clickable regions of the last frame, bottom to top
    pub hits: Vec<(Rect, Hit)>,
    drag: Option<Drag>,
    last_body_click: Option<(u64, Instant)>,
    pub wrap_cache: crate::ui::WrapCache,
}

pub fn open_url(url: &str) {
    if url.is_empty() {
        return;
    }
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(windows) {
        ("cmd", vec!["/c", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };
    let _ = std::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn hn_page(id: u64) -> String {
    format!("https://news.ycombinator.com/item?id={id}")
}

impl App {
    pub fn new(tx: UnboundedSender<Msg>, hn: Arc<Hn>, theme: ThemeName) -> App {
        let saved = store::load_saved();
        let history = store::load_history();
        let mut app = App {
            tx,
            hn,
            category: Category::Feed(FeedCategory::Top),
            refresh_key: 0,
            ids_key: None,
            ids_task: None,
            ids: vec![],
            ids_loading: true,
            ids_error: None,
            feed: ItemsSlot::default(),
            saved_slot: ItemsSlot::default(),
            history_slot: ItemsSlot::default(),
            saved_ids: saved.iter().map(|e| e.id).collect(),
            viewed_ids: history.iter().map(|e| e.id).collect(),
            saved,
            history,
            update_available: None,
            notice: None,
            notes: None,
            view: View::List,
            list_cursor: 0,
            detail_cursor: 0,
            collapsed: HashSet::new(),
            theme,
            popup: None,
            menu: None,
            help: false,
            stack: vec![],
            resolving: false,
            resolve_gen: 0,
            resolve_task: None,
            resolve_ref: None,
            pending_focus: None,
            comments_refresh: 0,
            last_g: None,
            comments_key: String::new(),
            tree_key: String::new(),
            tree: vec![],
            tree_task: None,
            flat: vec![],
            prefetch_task: None,
            prefetch_for: 0,
            popup_gen: 0,
            prev_items_len: 0,
            prev_flat: (0, false),
            prev_list_cursor: 0,
            prev_detail_cursor: 0,
            list_scroll: Scroll::default(),
            detail_scroll: Scroll::default(),
            status_spinner: Default::default(),
            list_spinner: Default::default(),
            comments_spinner: Default::default(),
            hits: vec![],
            drag: None,
            last_body_click: None,
            wrap_cache: Default::default(),
        };
        // one update check per launch, silently skipped when offline
        let tx = app.tx.clone();
        tokio::spawn(async move {
            let v = api::fetch_latest_version().await;
            let newer = v.filter(|v| api::compare_versions(v, VERSION).is_gt());
            let _ = tx.send(Msg::Update(newer));
        });
        app.list_scroll.reset();
        app
    }

    // ---- derived state -------------------------------------------------

    fn feed_category(&self) -> FeedCategory {
        match self.category {
            Category::Feed(f) => f,
            _ => FeedCategory::Top,
        }
    }

    pub fn items(&self) -> Vec<Arc<Item>> {
        // saved/history keep their own order; items that failed to load drop out
        let ordered = |slot: &ItemsSlot| {
            slot.key.iter().filter_map(|id| slot.items.iter().find(|i| i.id == *id).cloned()).collect::<Vec<_>>()
        };
        match self.category {
            Category::Saved => ordered(&self.saved_slot),
            Category::History => ordered(&self.history_slot),
            Category::Feed(_) => self.feed.items.clone(),
        }
    }

    pub fn list_loading(&self) -> bool {
        match self.category {
            Category::Saved => self.saved_slot.loading,
            Category::History => self.history_slot.loading,
            Category::Feed(_) => self.ids_loading || self.feed.loading,
        }
    }

    pub fn comments_loading(&self) -> bool {
        !self.comments_key.is_empty() && self.tree_key != self.comments_key
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn is_saved(&self, id: u64) -> bool {
        self.saved_ids.contains(&id)
    }

    // ---- persistence ---------------------------------------------------

    fn toggle_save(&mut self, id: u64) {
        if self.saved.iter().any(|e| e.id == id) {
            self.saved.retain(|e| e.id != id);
        } else {
            self.saved.insert(0, SavedEntry { id, saved_at: now_millis() });
        }
        self.saved_ids = self.saved.iter().map(|e| e.id).collect();
        store::persist_saved(&self.saved);
    }

    // move to the front (most recent), evict the oldest beyond the cap
    fn mark_viewed(&mut self, id: u64) {
        self.history.retain(|e| e.id != id);
        self.history.insert(0, HistoryEntry { id, viewed_at: now_millis() });
        self.history.truncate(HISTORY_CAP);
        self.viewed_ids = self.history.iter().map(|e| e.id).collect();
        store::persist_history(&self.history);
    }

    fn clear_history(&mut self) {
        self.history.clear();
        self.viewed_ids.clear();
        store::persist_history(&self.history);
    }

    // ---- navigation ----------------------------------------------------

    fn switch_category(&mut self, c: Category) {
        if c != self.category {
            self.list_scroll.reset();
        }
        self.category = c;
        self.list_cursor = 0;
    }

    fn cycle_category(&mut self, dir: i32) {
        let n = Category::ALL.len() as i32;
        let idx = Category::ALL.iter().position(|&c| c == self.category).unwrap_or(0) as i32;
        self.switch_category(Category::ALL[((idx + dir + n) % n) as usize]);
    }

    fn cancel_resolve(&mut self) {
        if let Some(t) = self.resolve_task.take() {
            t.abort();
        }
        self.resolve_gen += 1;
        self.resolving = false;
    }

    fn snapshot_current(&mut self) {
        if let View::Detail { story } = &self.view {
            self.stack.push(Snapshot { story: story.clone(), cursor: self.detail_cursor, collapsed: self.collapsed.clone() });
        }
    }

    // from the list starts fresh; from a detail view pushes it onto the stack
    fn enter_detail(&mut self, item: Arc<Item>, focus: Option<u64>) {
        self.cancel_resolve();
        self.mark_viewed(item.id);
        self.snapshot_current();
        self.view = View::Detail { story: item };
        self.detail_cursor = 0;
        self.collapsed.clear();
        self.pending_focus = focus;
        self.detail_scroll.reset_empty_mount();
    }

    // esc/h goes back ONE level
    fn pop_view(&mut self) {
        self.cancel_resolve();
        self.pending_focus = None;
        match self.stack.pop() {
            None => self.view = View::List,
            Some(top) => {
                self.view = View::Detail { story: top.story };
                self.detail_cursor = top.cursor;
                self.collapsed = top.collapsed;
                self.detail_scroll.reset_empty_mount();
            }
        }
    }

    fn go_home(&mut self) {
        self.cancel_resolve();
        self.pending_focus = None;
        self.stack.clear();
        self.view = View::List;
    }

    fn start_resolve(&mut self, r: HnItemRef) {
        self.cancel_resolve();
        self.resolving = true;
        self.resolve_ref = Some(r);
        let (hn, tx, gen) = (self.hn.clone(), self.tx.clone(), self.resolve_gen);
        self.resolve_task = Some(tokio::spawn(async move {
            let result = hn.resolve_story(r).await;
            let _ = tx.send(Msg::Resolved { gen, result });
        }));
    }

    fn open_link(&mut self, link: &Link) {
        if let Some(r) = parse_hn_item_link(&link.url) {
            self.popup = None;
            self.start_resolve(r);
        } else {
            open_url(&link.url);
        }
    }

    fn open_links_for(&mut self, idx: usize) {
        let Some(fc) = self.flat.get(idx) else { return };
        let links = extract_links(fc.node.item.text.as_deref());
        if !links.is_empty() {
            self.popup_gen += 1;
            self.popup = Some(Popup { links, cursor: 0 });
        }
    }

    fn toggle_collapse(&mut self, id: u64) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
    }

    fn page(&self, detail: bool) -> usize {
        let (vp, approx) = if detail { (self.detail_scroll.viewport, 4) } else { (self.list_scroll.viewport, 2) };
        let vp = if vp > 0 { vp } else { 20 };
        ((vp / approx) as usize).max(1)
    }

    fn double_g(&mut self) -> bool {
        let now = Instant::now();
        let hit = self.last_g.is_some_and(|t| now - t < Duration::from_millis(500));
        self.last_g = Some(now);
        hit
    }

    // ---- messages ------------------------------------------------------

    pub fn handle(&mut self, msg: Msg) -> Action {
        match msg {
            Msg::Key(k) => return self.key(k),
            Msg::Mouse(m) => return self.mouse(m),
            Msg::Resize => {}
            Msg::Ids { key, result } => {
                if self.ids_key == Some(key) {
                    self.ids_loading = false;
                    match result {
                        Ok(ids) => self.ids = ids,
                        Err(e) => self.ids_error = Some(e),
                    }
                }
            }
            Msg::Items { slot, key, items } => {
                let s = self.slot_mut(slot);
                if s.key == key {
                    s.items = items;
                    s.loading = false;
                }
            }
            Msg::Tree { key, tree } => {
                if key == self.comments_key {
                    self.tree_key = key;
                    self.tree = tree;
                }
            }
            Msg::Resolved { gen, result } => {
                if gen == self.resolve_gen {
                    self.resolve_task = None;
                    self.resolving = false;
                    match result {
                        Ok(r) => self.enter_detail(r.story, r.focus_id),
                        Err(error) => {
                            if let View::ResolveError { r, .. } = &self.view {
                                // retrying from an error view replaces it
                                let r = *r;
                                self.view = View::ResolveError { r, error };
                            } else if let Some(r) = self.resolve_ref {
                                self.snapshot_current();
                                self.view = View::ResolveError { r, error };
                            }
                        }
                    }
                }
            }
            Msg::Update(v) => self.update_available = v,
        }
        Action::None
    }

    fn slot_mut(&mut self, slot: Slot) -> &mut ItemsSlot {
        match slot {
            Slot::Feed => &mut self.feed,
            Slot::Saved => &mut self.saved_slot,
            Slot::History => &mut self.history_slot,
        }
    }

    fn key(&mut self, ev: KeyEvent) -> Action {
        if ev.kind != KeyEventKind::Press {
            return Action::None;
        }
        let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
        let mut shift = ev.modifiers.contains(KeyModifiers::SHIFT);
        // OpenTUI-style key names: letters lowercase + shift flag
        let name: String = match ev.code {
            KeyCode::Char(' ') => "space".into(),
            KeyCode::Char(c) if c.is_ascii_uppercase() => {
                shift = true;
                c.to_ascii_lowercase().to_string()
            }
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "return".into(),
            KeyCode::Esc => "escape".into(),
            KeyCode::Backspace => "backspace".into(),
            KeyCode::Tab => "tab".into(),
            KeyCode::BackTab => {
                shift = true;
                "tab".into()
            }
            KeyCode::Up => "up".into(),
            KeyCode::Down => "down".into(),
            KeyCode::Left => "left".into(),
            KeyCode::Right => "right".into(),
            KeyCode::PageUp => "pageup".into(),
            KeyCode::PageDown => "pagedown".into(),
            _ => return Action::None,
        };
        let n = name.as_str();

        if n == "q" || (ctrl && n == "c") {
            return Action::Quit;
        }
        let is_help = n == "?" || (n == "/" && shift);
        let back = matches!(n, "escape" | "backspace");

        if self.help {
            if back || is_help {
                self.help = false;
            }
            return Action::None;
        }

        if let Some(nv) = &mut self.notes {
            let page = 10;
            match n {
                "j" | "down" => nv.scroll += 1,
                "k" | "up" => nv.scroll -= 1,
                "g" if shift => nv.scroll = nv.max,
                "g" => nv.scroll = 0,
                "space" | "pagedown" => nv.scroll += page,
                "d" if ctrl => nv.scroll += page,
                "u" if ctrl => nv.scroll -= page,
                "pageup" => nv.scroll -= page,
                "return" => self.notes = None,
                _ if back => self.notes = None,
                _ => {}
            }
            if let Some(nv) = &mut self.notes {
                nv.scroll = nv.scroll.clamp(0, nv.max.max(0));
            }
            return Action::None;
        }

        if let Some(m) = &mut self.menu {
            let max = m.items.len().saturating_sub(1);
            match n {
                "j" | "down" => m.cursor = (m.cursor + 1).min(max),
                "k" | "up" => m.cursor = m.cursor.saturating_sub(1),
                "return" => self.activate_menu_item(None),
                _ if back => self.menu = None,
                _ => {}
            }
            return Action::None;
        }

        if let Some(p) = &mut self.popup {
            let max = p.links.len().saturating_sub(1);
            match n {
                "j" | "down" => p.cursor = (p.cursor + 1).min(max),
                "k" | "up" => p.cursor = p.cursor.saturating_sub(1),
                "g" if shift => p.cursor = max,
                "g" => p.cursor = 0,
                "return" => {
                    if let Some(l) = p.links.get(p.cursor).cloned() {
                        self.open_link(&l);
                    }
                }
                "o" => {
                    if let Some(l) = p.links.get(p.cursor) {
                        open_url(&l.url);
                    }
                }
                "y" if shift => {
                    if let Some(l) = p.links.get(p.cursor) {
                        let url = l.url.clone();
                        self.copy(&url, "link");
                    }
                }
                _ if back => self.popup = None,
                _ => {}
            }
            return Action::None;
        }

        // esc while a link is resolving cancels it (aborts the fetch chain)
        if self.resolving && back {
            self.cancel_resolve();
            return Action::None;
        }
        if is_help {
            self.help = true;
            return Action::None;
        }
        if n == "t" {
            self.theme = self.theme.toggled();
            let mut st = store::load_state();
            st.theme = Some(self.theme);
            store::persist_state(&st);
            return Action::ThemeWipe(self.theme.theme().accent);
        }
        if n == "s" && shift {
            self.go_home();
            self.switch_category(Category::Saved);
            return Action::None;
        }
        if n == "h" && shift {
            self.go_home();
            self.switch_category(Category::History);
            return Action::None;
        }

        match &self.view {
            View::List => self.list_key(n, shift, ctrl),
            View::ResolveError { r, error } => {
                let (r, gone) = (*r, matches!(error, ResolveError::Gone));
                if matches!(n, "h" | "left" | "backspace" | "escape") {
                    self.pop_view();
                } else if n == "r" && !gone {
                    // a gone post stays gone — only transient failures retry
                    self.start_resolve(r);
                }
            }
            View::Detail { story } => {
                let story = story.clone();
                self.detail_key(n, shift, ctrl, story);
            }
        }
        Action::None
    }

    fn list_key(&mut self, n: &str, shift: bool, ctrl: bool) {
        let items = self.items();
        let max = items.len().saturating_sub(1);
        let pg = self.page(false);
        let cur = items.get(self.list_cursor).cloned();
        match n {
            "j" | "down" => self.list_cursor = (self.list_cursor + 1).min(max),
            "k" | "up" => self.list_cursor = self.list_cursor.saturating_sub(1),
            "g" if shift => self.list_cursor = max,
            "g" => {
                if self.double_g() {
                    self.list_cursor = 0
                }
            }
            "d" if ctrl => self.list_cursor = (self.list_cursor + pg).min(max),
            "pagedown" => self.list_cursor = (self.list_cursor + pg).min(max),
            "u" if ctrl => self.list_cursor = self.list_cursor.saturating_sub(pg),
            "pageup" => self.list_cursor = self.list_cursor.saturating_sub(pg),
            "c" | "return" => {
                if let Some(c) = cur {
                    self.enter_detail(c, None)
                }
            }
            "h" | "left" => self.cycle_category(-1),
            "l" | "right" => self.cycle_category(1),
            "tab" => self.cycle_category(if shift { -1 } else { 1 }),
            "o" => {
                if let Some(c) = cur {
                    if let Some(u) = &c.url {
                        self.mark_viewed(c.id);
                        open_url(u);
                    }
                }
            }
            "y" if shift => {
                if let Some(c) = cur {
                    self.copy_story_link(&c);
                }
            }
            "y" => {
                if let Some(c) = cur {
                    self.mark_viewed(c.id);
                    open_url(&hn_page(c.id));
                }
            }
            "s" => {
                if let Some(c) = cur {
                    self.toggle_save(c.id)
                }
            }
            "x" if self.category == Category::History => self.clear_history(),
            "1" | "2" | "3" | "4" | "5" | "6" => {
                let i: usize = n.parse().unwrap();
                self.switch_category(Category::Feed(FeedCategory::ALL[i - 1]));
            }
            "r" if matches!(self.category, Category::Feed(_)) => self.refresh_key += 1,
            _ => {}
        }
    }

    fn detail_key(&mut self, n: &str, shift: bool, ctrl: bool, story: Arc<Item>) {
        let max = self.flat.len().saturating_sub(1);
        let pg = self.page(true);
        match n {
            "j" if shift => {
                if let Some(i) = tree::next_root_index(&self.flat, self.detail_cursor, true) {
                    self.detail_cursor = i
                }
            }
            "k" if shift => {
                if let Some(i) = tree::next_root_index(&self.flat, self.detail_cursor, false) {
                    self.detail_cursor = i
                }
            }
            "j" | "down" => self.detail_cursor = (self.detail_cursor + 1).min(max),
            "k" | "up" => self.detail_cursor = self.detail_cursor.saturating_sub(1),
            "g" if shift => self.detail_cursor = max,
            "g" => {
                if self.double_g() {
                    self.detail_cursor = 0
                }
            }
            "d" if ctrl => self.detail_cursor = (self.detail_cursor + pg).min(max),
            "pagedown" => self.detail_cursor = (self.detail_cursor + pg).min(max),
            "u" if ctrl => self.detail_cursor = self.detail_cursor.saturating_sub(pg),
            "pageup" => self.detail_cursor = self.detail_cursor.saturating_sub(pg),
            "space" => {
                if let Some(c) = self.flat.get(self.detail_cursor) {
                    let id = c.node.item.id;
                    self.toggle_collapse(id)
                }
            }
            "c" => {
                // fold/unfold the whole thread you're in, landing on its root
                if let Some(ri) = tree::root_index_of(&self.flat, self.detail_cursor) {
                    let id = self.flat[ri].node.item.id;
                    self.toggle_collapse(id);
                    self.detail_cursor = ri;
                }
            }
            "return" => self.open_links_for(self.detail_cursor),
            "o" => {
                if let Some(u) = &story.url {
                    open_url(u)
                }
            }
            "y" if shift => match self.flat.get(self.detail_cursor).map(|f| f.node.item.clone()) {
                Some(c) => self.copy(&html_to_text(c.text.as_deref()), &format!("comment by {}", c.by.as_deref().unwrap_or("?"))),
                None => self.copy_story_link(&story),
            },
            "y" => open_url(&hn_page(story.id)),
            "s" => self.toggle_save(story.id),
            "r" => {
                // retry comments only when they failed to load entirely
                if !self.comments_loading() && self.flat.is_empty() && story.descendants.unwrap_or(0) > 0 {
                    self.comments_refresh += 1;
                }
            }
            "h" | "left" | "backspace" | "escape" => self.pop_view(),
            _ => {}
        }
    }

    // ---- mouse ---------------------------------------------------------
    //
    // Ports the onMouseDown/onMouseOver handlers of the TSX components. The
    // renderer records what's clickable where (`hits`); this walks that
    // stack from the top, so overlays shadow what's under them.

    fn hits_at(&self, x: i32, y: i32) -> Vec<(Rect, Hit)> {
        self.hits.iter().rev().filter(|(r, _)| r.contains(x, y)).copied().collect()
    }

    fn scroll_mut(&mut self, p: Pane) -> &mut Scroll {
        match p {
            Pane::Detail => &mut self.detail_scroll,
            // the notes overlay scrolls by itself (see `mouse`)
            Pane::List | Pane::Notes => &mut self.list_scroll,
        }
    }

    fn mouse(&mut self, ev: MouseEvent) -> Action {
        let (x, y) = (ev.column as i32, ev.row as i32);
        match ev.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                // shift+wheel scrolls sideways in OpenTUI, and nothing here does
                if ev.modifiers.contains(KeyModifiers::SHIFT) {
                    return Action::Unchanged;
                }
                let dir = if ev.kind == MouseEventKind::ScrollUp { -1 } else { 1 };
                for (_, h) in self.hits_at(x, y) {
                    match h {
                        Hit::ScrollArea(Pane::Notes) => {
                            let Some(nv) = &mut self.notes else { break };
                            let to = (nv.scroll + dir).clamp(0, nv.max.max(0));
                            let moved = to != nv.scroll;
                            nv.scroll = to;
                            return if moved { Action::None } else { Action::Unchanged };
                        }
                        Hit::ScrollArea(p) | Hit::Scrollbar(p) => {
                            return if self.scroll_mut(p).wheel(dir) { Action::None } else { Action::Unchanged };
                        }
                        Hit::Backdrop(_) | Hit::Panel => break,
                        _ => {}
                    }
                }
                Action::Unchanged
            }
            MouseEventKind::Down(button) => self.mouse_down(x, y, button),
            MouseEventKind::Drag(MouseButton::Left) if self.drag.is_some() => {
                let d = self.drag.as_ref().unwrap();
                let (pane, track, offset) = (d.pane, d.track, d.offset);
                self.scroll_mut(pane).slider_drag(y - track.y, track.h, offset);
                Action::None
            }
            MouseEventKind::Up(_) if self.drag.is_some() => {
                let d = self.drag.take().unwrap();
                self.scroll_mut(d.pane).slider_drag(y - d.track.y, d.track.h, d.offset);
                Action::None
            }
            MouseEventKind::Moved | MouseEventKind::Drag(_) => self.hover(x, y),
            _ => Action::Unchanged,
        }
    }

    // the context menu highlights whatever item the pointer enters
    fn hover(&mut self, x: i32, y: i32) -> Action {
        let over = match self.hits_at(x, y).first() {
            Some((_, Hit::MenuItem(i))) => Some(*i),
            _ => None,
        };
        let Some(m) = &mut self.menu else { return Action::Unchanged };
        if over == m.hover {
            return Action::Unchanged;
        }
        m.hover = over;
        match over {
            Some(i) if i != m.cursor => {
                m.cursor = i;
                Action::None
            }
            _ => Action::Unchanged,
        }
    }

    fn mouse_down(&mut self, x: i32, y: i32, button: MouseButton) -> Action {
        for (r, h) in self.hits_at(x, y) {
            match h {
                Hit::Backdrop(o) => match o {
                    Overlay::Help => self.help = false,
                    Overlay::Links => self.popup = None,
                    Overlay::Menu => self.menu = None,
                    Overlay::Notes => self.notes = None,
                },
                Hit::Panel => {}
                Hit::MenuItem(i) => self.activate_menu_item(Some(i)),
                Hit::LinkRow(i) => {
                    // a click on the highlighted row opens it; elsewhere it highlights
                    let Some(p) = &mut self.popup else { break };
                    if p.cursor == i {
                        if let Some(l) = p.links.get(i).cloned() {
                            self.open_link(&l);
                        }
                    } else {
                        p.cursor = i;
                    }
                }
                Hit::Home => self.on_home(),
                Hit::Tab(c) => self.switch_category(c),
                Hit::Row(i) => {
                    let Some(item) = self.items().get(i).cloned() else { break };
                    if button == MouseButton::Right {
                        self.list_cursor = i;
                        self.open_menu(item, x, y);
                    } else if i == self.list_cursor {
                        self.enter_detail(item, None);
                    } else {
                        self.list_cursor = i;
                    }
                }
                Hit::CommentHead(i) => {
                    let Some(id) = self.flat.get(i).map(|f| f.node.item.id) else { break };
                    self.detail_cursor = i;
                    self.toggle_collapse(id);
                }
                Hit::CommentBody(i) => {
                    let Some(id) = self.flat.get(i).map(|f| f.node.item.id) else { break };
                    let now = Instant::now();
                    let double = self.last_body_click.is_some_and(|(cid, t)| cid == id && now - t < DOUBLE_CLICK);
                    self.last_body_click = Some((id, now));
                    if double {
                        self.open_links_for(i);
                    } else {
                        self.detail_cursor = i;
                    }
                }
                Hit::StoryUrl => {
                    if let View::Detail { story } = &self.view {
                        if let Some(u) = &story.url {
                            open_url(u);
                        }
                    }
                }
                Hit::Scrollbar(p) => {
                    let s = self.scroll_mut(p);
                    let rel = y - r.y;
                    let (ts, th) = s.thumb_rows(r.h);
                    if !(ts..ts + th).contains(&rel) {
                        s.slider_jump(rel, r.h);
                    }
                    let offset = s.drag_offset(rel, r.h);
                    self.drag = Some(Drag { pane: p, track: r, offset });
                }
                // the scroll container itself doesn't take clicks; keep bubbling
                Hit::ScrollArea(_) => continue,
            }
            return Action::None;
        }
        Action::Unchanged
    }

    // the Y tile: back to the list, or refresh it
    fn on_home(&mut self) {
        if !matches!(self.view, View::List) {
            self.go_home();
        } else if matches!(self.category, Category::Saved | Category::History) {
            // local lists: nothing to refresh, just back to the top
            self.list_cursor = 0;
        } else {
            self.refresh_key += 1;
        }
    }

    fn open_menu(&mut self, item: Arc<Item>, x: i32, y: i32) {
        let mut items = vec![MenuItem {
            label: if self.is_saved(item.id) { "★ Unsave" } else { "☆ Save" }.into(),
            action: MenuAction::ToggleSave(item.id),
        }];
        if let Some(u) = &item.url {
            items.push(MenuItem { label: "Open URL in browser".into(), action: MenuAction::OpenUrl(item.id, u.clone()) });
        }
        let link = item.url.clone().unwrap_or_else(|| hn_page(item.id));
        items.push(MenuItem { label: "Open comments".into(), action: MenuAction::OpenComments(item) });
        items.push(MenuItem { label: "Copy link".into(), action: MenuAction::Copy(link) });
        self.menu = Some(Menu { x, y, items, cursor: 0, hover: None });
    }

    // runs the item under the cursor (keyboard) or the one clicked, and closes
    fn activate_menu_item(&mut self, idx: Option<usize>) {
        let Some(m) = self.menu.take() else { return };
        let Some(item) = m.items.into_iter().nth(idx.unwrap_or(m.cursor)) else { return };
        match item.action {
            MenuAction::ToggleSave(id) => self.toggle_save(id),
            MenuAction::OpenUrl(id, url) => {
                self.mark_viewed(id);
                open_url(&url);
            }
            MenuAction::OpenComments(story) => self.enter_detail(story, None),
            MenuAction::Copy(link) => self.copy(&link, "link"),
        }
    }

    // ---- copy ----------------------------------------------------------

    fn copy(&mut self, text: &str, what: &str) {
        let how = match clipboard::copy(text) {
            clipboard::Via::Tool => "",
            // OSC 52: the terminal may or may not honour it, so say how
            clipboard::Via::Terminal => " (via the terminal)",
        };
        self.notify(format!("copied {what}{how}"), false, Duration::from_millis(2500));
    }

    // the article if there is one, else the HN page (Ask HN and friends)
    fn copy_story_link(&mut self, story: &Item) {
        match &story.url {
            Some(u) => self.copy(u, "link"),
            None => self.copy(&hn_page(story.id), "HN link"),
        }
    }

    // ---- effects -------------------------------------------------------

    fn load_items(&mut self, slot: Slot, ids: Vec<u64>) {
        let (hn, tx) = (self.hn.clone(), self.tx.clone());
        let s = self.slot_mut(slot);
        if s.key == ids {
            return;
        }
        if let Some(t) = s.task.take() {
            t.abort();
        }
        s.key = ids.clone();
        s.items.clear();
        if ids.is_empty() {
            return;
        }
        s.loading = true;
        s.task = Some(tokio::spawn(async move {
            let items = hn.fetch_items(ids.clone(), 10).await;
            let _ = tx.send(Msg::Items { slot, key: ids, items });
        }));
    }

    /// Run after every batch of messages: start/abort whatever the new state
    /// calls for, then apply the derived-state "effects".
    pub fn sync(&mut self) {
        // feed ids
        let key = (self.feed_category(), self.refresh_key);
        if self.ids_key != Some(key) {
            if let Some(t) = self.ids_task.take() {
                t.abort();
            }
            self.ids_key = Some(key);
            self.ids.clear();
            self.ids_loading = true;
            self.ids_error = None;
            let (hn, tx) = (self.hn.clone(), self.tx.clone());
            self.ids_task = Some(tokio::spawn(async move {
                let result = hn.fetch_ids(key.0).await;
                let _ = tx.send(Msg::Ids { key, result });
            }));
        }

        let visible: Vec<u64> = self.ids.iter().take(PAGE_SIZE).copied().collect();
        self.load_items(Slot::Feed, visible);
        let saved = if self.category == Category::Saved { self.saved.iter().map(|e| e.id).collect() } else { vec![] };
        self.load_items(Slot::Saved, saved);
        let hist = if self.category == Category::History {
            self.history.iter().take(PAGE_SIZE).map(|e| e.id).collect()
        } else {
            vec![]
        };
        self.load_items(Slot::History, hist);

        // comments: keyed by story kids + refresh counter
        let kids = match &self.view {
            View::Detail { story } => story.kids.clone().unwrap_or_default(),
            _ => vec![],
        };
        let ckey = if kids.is_empty() {
            String::new()
        } else {
            format!("{}|{}", self.comments_refresh, kids.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(","))
        };
        if ckey != self.comments_key {
            if let Some(t) = self.tree_task.take() {
                t.abort();
            }
            self.comments_key = ckey.clone();
            if !ckey.is_empty() {
                let (hn, tx) = (self.hn.clone(), self.tx.clone());
                self.tree_task = Some(tokio::spawn(async move {
                    let tree = tree::load_forest(hn, kids, 8).await;
                    let _ = tx.send(Msg::Tree { key: ckey, tree });
                }));
            }
        }
        let fresh = !self.comments_key.is_empty() && self.tree_key == self.comments_key;
        let tree: &[Arc<Node>] = if fresh { &self.tree } else { &[] };
        let flat = tree::flatten(tree, &self.collapsed);
        let flat_changed = flat.len() != self.flat.len()
            || flat.iter().zip(&self.flat).any(|(a, b)| !Arc::ptr_eq(&a.node, &b.node) || a.hidden_children != b.hidden_children);
        self.flat = flat;

        // speculative prefetch while the links popup is open, so ⏎ on an
        // internal HN link is instant; closing the popup aborts it
        let want = if self.popup.is_some() { self.popup_gen } else { 0 };
        if want != self.prefetch_for {
            if let Some(t) = self.prefetch_task.take() {
                t.abort();
            }
            self.prefetch_for = want;
            if let Some(p) = &self.popup {
                let refs: Vec<HnItemRef> = p.links.iter().filter_map(|l| parse_hn_item_link(&l.url)).collect();
                if !refs.is_empty() {
                    let hn = self.hn.clone();
                    self.prefetch_task = Some(tokio::spawn(async move {
                        use futures::StreamExt;
                        futures::stream::iter(refs)
                            .for_each_concurrent(4, |r| {
                                let hn = hn.clone();
                                async move {
                                    let _ = hn.resolve_story(r).await;
                                }
                            })
                            .await;
                    }));
                }
            }
        }

        // clamp the list cursor when the list shrinks
        let n = self.items().len();
        if n != self.prev_items_len {
            if self.list_cursor >= n {
                self.list_cursor = n.saturating_sub(1);
            }
            self.list_scroll.pending_into_view = true;
            self.prev_items_len = n;
        }
        // don't clamp while comments (re)load — a restored cursor must survive
        let loading = self.comments_loading();
        if (self.flat.len(), loading) != self.prev_flat {
            if !loading && self.detail_cursor >= self.flat.len() {
                self.detail_cursor = self.flat.len().saturating_sub(1);
            }
            self.prev_flat = (self.flat.len(), loading);
        }
        // after following an internal link, land on the linked comment
        if let Some(id) = self.pending_focus {
            if !loading {
                if let Some(i) = self.flat.iter().position(|f| f.node.item.id == id) {
                    self.detail_cursor = i;
                }
                self.pending_focus = None;
            }
        }
        if self.list_cursor != self.prev_list_cursor {
            self.list_scroll.pending_into_view = true;
            self.prev_list_cursor = self.list_cursor;
        }
        if self.detail_cursor != self.prev_detail_cursor || flat_changed {
            self.detail_scroll.pending_into_view = true;
            self.prev_detail_cursor = self.detail_cursor;
        }
    }

    pub fn notify(&mut self, text: impl Into<String>, warn: bool, for_: Duration) {
        self.notice = Some(Notice { text: text.into(), warn, until: Instant::now() + for_ });
    }

    /// The visible notice, if any; drops it once it has expired.
    pub fn current_notice(&mut self) -> Option<&Notice> {
        if self.notice.as_ref().is_some_and(|n| Instant::now() >= n.until) {
            self.notice = None;
        }
        self.notice.as_ref()
    }

    /// When the screen next needs a redraw on its own (a notice expiring).
    pub fn notice_deadline(&self) -> Option<Instant> {
        self.notice.as_ref().map(|n| n.until)
    }

    pub fn spinners_active(&self) -> bool {
        self.status_spinner.active() || self.list_spinner.active() || self.comments_spinner.active()
    }
}
