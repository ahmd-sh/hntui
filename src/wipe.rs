// Animated theme switch (port of src/utils/themeWipe.ts): a wave sweeps out
// of a random point on a random screen edge and every cell flips to the new
// theme as the front passes it.
//
// Same trick as the TS version, and arguably more natural here: snapshot the
// last OLD-theme frame, render the NEW theme normally, then for 500ms paint
// the snapshot back over every cell the wave hasn't reached yet. In ratatui
// the frame is plain data you own, so there is no post-process hook to find.

use crate::canvas::Canvas;
use crate::color::Rgba;
use std::time::{Duration, Instant};

const DURATION: Duration = Duration::from_millis(500);
const EDGE_WIDTH: f32 = 1.5;
const X_SCALE: f32 = 0.55;

pub struct Wipe {
    start: Instant,
    snap: Canvas,
    dist: Vec<f32>,
    max_dist: f32,
    accent: Rgba,
}

impl Wipe {
    pub fn new(snap: Canvas, accent: Rgba) -> Wipe {
        let (w, h) = (snap.w, snap.h);
        // a fresh origin per wipe: anywhere along any edge
        let edge = fastrand::u8(0..4);
        let ox = match edge {
            1 => (w - 1) as f32,
            3 => 0.0,
            _ => fastrand::f32() * (w - 1) as f32,
        };
        let oy = match edge {
            0 => 0.0,
            2 => (h - 1) as f32,
            _ => fastrand::f32() * (h - 1) as f32,
        };
        let mut dist = Vec::with_capacity((w * h) as usize);
        let mut max_dist = 0f32;
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 - ox) * X_SCALE;
                let dy = y as f32 - oy;
                let d = (dx * dx + dy * dy).sqrt();
                max_dist = max_dist.max(d);
                dist.push(d);
            }
        }
        Wipe { start: Instant::now(), snap, dist, max_dist, accent }
    }

    /// Paint over the freshly rendered frame. Returns false once finished
    /// (or when a resize made the snapshot meaningless).
    pub fn apply(&self, live: &mut Canvas) -> bool {
        if live.w != self.snap.w || live.h != self.snap.h {
            return false;
        }
        let t = (self.start.elapsed().as_secs_f32() / DURATION.as_secs_f32()).min(1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t); // ease-out: fast ignition, soft landing
        let r = eased * (self.max_dist + EDGE_WIDTH);
        for i in 0..self.dist.len() {
            let d = self.dist[i];
            if d <= r - EDGE_WIDTH {
                continue; // wave passed: new theme shows through
            }
            let (x, y) = (i as i32 % live.w, i as i32 / live.w);
            if d <= r {
                // wavefront: the just-flipped cell glows on an accent background
                let c = live.cells[i].clone();
                live.put(x, y, &c.sym, if c.cont { 0 } else { 1 }, c.fg, self.accent, c.bold);
            } else {
                // not reached yet: hold the old-theme snapshot in place
                let c = &self.snap.cells[i];
                live.put(x, y, &c.sym, 1, c.fg, c.bg, c.bold);
            }
        }
        t < 1.0
    }
}
