// Colors and alpha compositing, matched to OpenTUI's native renderer so the
// port produces the exact same terminal colors (measured, not guessed: see
// the comparison notes). Components are floats in 0..=1, like OpenTUI's RGBA.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const WHITE: Rgba = Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    /// "#rrggbb" or "#rrggbbaa", usable in consts.
    pub const fn hex(s: &str) -> Rgba {
        let b = s.as_bytes();
        let a = if b.len() == 9 { byte(b, 7) } else { 1.0 };
        Rgba { r: byte(b, 1), g: byte(b, 3), b: byte(b, 5), a }
    }

    pub fn with_alpha(self, a: f32) -> Rgba {
        Rgba { a, ..self }
    }

    pub fn to_u8(self) -> (u8, u8, u8) {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        (c(self.r), c(self.g), c(self.b))
    }
}

const fn byte(b: &[u8], i: usize) -> f32 {
    (nib(b[i]) << 4 | nib(b[i + 1])) as f32 / 255.0
}

const fn nib(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

// OpenTUI approximates pow() by scaling the float's bit pattern (a
// Schraudolph-style trick). It is piecewise-linear between powers of two, so
// a "real" powf gives visibly different colors (39 vs 40 on the row highlight).
fn fast_pow(base: f32, exp: f32) -> f32 {
    if base <= 0.0 {
        return 0.0;
    }
    const ONE: i32 = 0x3f80_0000; // bits of 1.0f32
    let bits = base.to_bits() as i32;
    let out = ((bits - ONE) as f32 * exp) as i32 + ONE;
    f32::from_bits(out as u32)
}

// alpha → how much of the overlay actually shows: a gentle curve that makes
// low alphas a touch stronger and snaps high alphas toward opaque.
fn perceptual(alpha: f32) -> f32 {
    if alpha > 0.8 {
        0.8 + fast_pow((alpha - 0.8) * 5.0, 0.2) * 0.2
    } else {
        fast_pow(alpha, 0.9)
    }
}

/// Composite `over` onto `under`. The result keeps the overlay's alpha, the
/// way OpenTUI's buffer does — it matters when a snapshot is redrawn on top
/// of a live frame (the theme wipe).
pub fn blend(over: Rgba, under: Rgba) -> Rgba {
    if over.a >= 1.0 {
        return over;
    }
    if over.a <= 0.0 {
        return under;
    }
    let p = perceptual(over.a);
    let q = 1.0 - p;
    Rgba {
        r: over.r * p + under.r * q,
        g: over.g * p + under.g * q,
        b: over.b * p + under.b * q,
        a: over.a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(c: Rgba) -> f32 {
        c.r * 255.0
    }

    // Values captured from OpenTUI's own buffer (white@alpha over black).
    #[test]
    fn matches_opentui_blend_curve() {
        let black = Rgba::hex("#000000");
        for (a, want) in [
            (8u8, 11.981f32),
            (26, 31.369),
            (128, 140.7),
            (136, 147.9),
            (204, 209.1),
            (205, 227.6),
            (240, 252.0),
        ] {
            let got = v(blend(Rgba::WHITE.with_alpha(a as f32 / 255.0), black));
            assert!((got - want).abs() < 0.05, "alpha {a}: got {got}, want {want}");
        }
    }

    #[test]
    fn dark_body_and_row_highlight() {
        let body = blend(Rgba::hex("#0a0a0acc"), Rgba::TRANSPARENT);
        assert_eq!(body.to_u8(), (8, 8, 8));
        let row = blend(Rgba::hex("#ffffff1a"), body);
        assert_eq!(row.to_u8(), (39, 39, 39));
    }
}
