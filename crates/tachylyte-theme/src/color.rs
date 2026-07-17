//! Small, allocation-free colour conversion helpers.
//!
//! GPUI's [`gpui::Hsla`] stores hue, saturation, lightness, and alpha as
//! normalized `f32`s: hue is one turn (`0.0..=1.0`), while the other channels
//! are `0.0..=1.0`.  The hexadecimal helpers below use ordinary 8-bit channel
//! semantics (`0xRRGGBB` and `0xRRGGBBAA`), rather than GPUI's normalized
//! representation.

use gpui::Hsla;

/// Convert an RGB integer written as `0xRRGGBB` to an opaque GPUI colour.
pub const fn hex_rgb(rgb: u32) -> Hsla {
    hex_rgba((rgb << 8) | 0xff)
}

/// Convert an RGBA integer written as `0xRRGGBBAA` to a GPUI colour.
pub const fn hex_rgba(rgba: u32) -> Hsla {
    rgba8(
        ((rgba >> 24) & 0xff) as u8,
        ((rgba >> 16) & 0xff) as u8,
        ((rgba >> 8) & 0xff) as u8,
        (rgba & 0xff) as u8,
    )
}

/// Convert 8-bit RGBA channels to GPUI's normalized HSL representation.
pub const fn rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Hsla {
    let r = red as f32 / 255.0;
    let g = green as f32 / 255.0;
    let b = blue as f32 / 255.0;
    let max = max(max(r, g), b);
    let min = min(min(r, g), b);
    let lightness = (max + min) / 2.0;
    let (hue, saturation) = if max == min {
        (0.0, 0.0)
    } else {
        let delta = max - min;
        let saturation = if lightness > 0.5 {
            delta / (2.0 - max - min)
        } else {
            delta / (max + min)
        };
        let mut hue = if max == r {
            (g - b) / delta + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        hue /= 6.0;
        (hue, saturation)
    };
    Hsla {
        h: hue,
        s: saturation,
        l: lightness,
        a: alpha as f32 / 255.0,
    }
}

/// Extension methods for converting GPUI colours back to packed 8-bit RGB.
pub trait HslaColorExt {
    /// Return `0xRRGGBB`; alpha is ignored.
    fn to_rgb24(self) -> u32;
    /// Return `0xRRGGBBAA`.
    fn to_rgba32(self) -> u32;
}

impl HslaColorExt for Hsla {
    fn to_rgb24(self) -> u32 {
        self.to_rgba32() >> 8
    }

    fn to_rgba32(self) -> u32 {
        let (r, g, b) = hsl_to_rgb(self.h, self.s, self.l);
        (channel(r) << 24) | (channel(g) << 16) | (channel(b) << 8) | channel(self.a)
    }
}

const fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    (
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    )
}

const fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

const fn channel(value: f32) -> u32 {
    (clamp(value, 0.0, 1.0) * 255.0 + 0.5) as u32
}
const fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}
const fn min(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}
const fn max(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}
