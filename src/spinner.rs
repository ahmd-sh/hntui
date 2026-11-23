// The knight-rider loader from src/spinner.ts + Loader.tsx: an 8-cell block
// scanner sweeping out and back in HN orange, with hold frames at each end.

use crate::color::Rgba;
use std::time::{Duration, Instant};

const WIDTH: usize = 8;
const HOLD_START: usize = 30;
const HOLD_END: usize = 9;
pub const TOTAL_FRAMES: usize = WIDTH + HOLD_END + (WIDTH - 1) + HOLD_START;
pub const INTERVAL: Duration = Duration::from_millis(40);

const RAMP: [Rgba; 6] = [
    Rgba::hex("#ff6600"),
    Rgba::hex("#ffaa66"),
    Rgba::hex("#dd5500"),
    Rgba::hex("#aa4400"),
    Rgba::hex("#773300"),
    Rgba::hex("#442200"),
];
const INACTIVE: Rgba = Rgba::hex("#2a1500");

struct State {
    active: i64,
    holding: bool,
    hold_progress: i64,
    hold_total: i64,
    move_progress: i64,
    move_total: i64,
    forward: bool,
}

fn state(frame: usize) -> State {
    let (f, w) = (frame as i64, WIDTH as i64);
    let (fwd, hold_end, back) = (w, HOLD_END as i64, w - 1);
    if f < fwd {
        State { active: f, holding: false, hold_progress: 0, hold_total: 0, move_progress: f, move_total: fwd, forward: true }
    } else if f < fwd + hold_end {
        State { active: w - 1, holding: true, hold_progress: f - fwd, hold_total: hold_end, move_progress: 0, move_total: 0, forward: true }
    } else if f < fwd + hold_end + back {
        let bi = f - fwd - hold_end;
        State { active: w - 2 - bi, holding: false, hold_progress: 0, hold_total: 0, move_progress: bi, move_total: back, forward: false }
    } else {
        State {
            active: 0,
            holding: true,
            hold_progress: f - fwd - hold_end - back,
            hold_total: HOLD_START as i64,
            move_progress: 0,
            move_total: 0,
            forward: false,
        }
    }
}

fn color_index(frame: usize, ch: usize) -> i64 {
    let s = state(frame);
    let c = ch as i64;
    let dist = if s.forward { s.active - c } else { c - s.active };
    if s.holding {
        return dist + s.hold_progress;
    }
    if dist > 0 && dist < RAMP.len() as i64 {
        return dist;
    }
    if dist == 0 {
        0
    } else {
        -1
    }
}

/// (glyph, color) for each of the 8 cells at `frame`.
pub fn frame(frame: usize) -> [(&'static str, Rgba); WIDTH] {
    let s = state(frame);
    // the trail fades in while moving and out while holding
    let alpha = if s.holding && s.hold_total > 0 {
        (1.0 - (s.hold_progress as f32 / s.hold_total as f32).min(1.0)).max(0.0)
    } else if !s.holding && s.move_total > 0 {
        (s.move_progress as f32 / (s.move_total - 1).max(1) as f32).min(1.0)
    } else {
        1.0
    };
    let mut out = [("⬝", INACTIVE); WIDTH];
    for (i, cell) in out.iter_mut().enumerate() {
        let idx = color_index(frame, i);
        let glyph = if idx >= 0 && (idx as usize) < RAMP.len() { "■" } else { "⬝" };
        let color = if idx >= 0 && (idx as usize) < RAMP.len() { RAMP[idx as usize] } else { INACTIVE.with_alpha(alpha) };
        *cell = (glyph, color);
    }
    out
}

/// Each on-screen loader animates from frame 0 when it first appears.
#[derive(Default)]
pub struct Clock {
    started: Option<Instant>,
    seen_this_frame: bool,
}

impl Clock {
    pub fn tick(&mut self, now: Instant) -> usize {
        self.seen_this_frame = true;
        let start = *self.started.get_or_insert(now);
        ((now - start).as_millis() / INTERVAL.as_millis()) as usize % TOTAL_FRAMES
    }

    /// call after each frame: loaders not drawn this frame reset
    pub fn end_frame(&mut self) {
        if !self.seen_this_frame {
            self.started = None;
        }
        self.seen_this_frame = false;
    }

    pub fn active(&self) -> bool {
        self.started.is_some()
    }
}
