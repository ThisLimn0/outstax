//! Colour math for the four photo adjustments, ported from LightCraft
//! (<https://github.com/storytold/lightcraft>), Copyright (c) 2026 ArtCraft Team and the
//! LightCraft contributors, licensed MIT OR Apache-2.0 and used here under MIT (see
//! `licenses/LightCraft-MIT.txt`). Modified for this project: the working space is linear sRGB
//! (LightCraft's is linear Rec.2020) and the sliders run on this app's −100..100 scale.
//! [`crate::imaging::adjust`] calls these in LightCraft's stage order.

type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

#[inline]
fn apply_f32(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

// Ported from LightCraft crates/color/src/lib.rs `Mat3::inverse` (MIT OR Apache-2.0)
fn inverse(m: &M3) -> M3 {
    let d = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let i = 1.0 / d;
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * i,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * i,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * i,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * i,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * i,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * i,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * i,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * i,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * i,
        ],
    ]
}

// ---------------------------------------------------------------- transfer

// Ported from LightCraft crates/color/src/transfer.rs `srgb_to_linear` (MIT OR Apache-2.0)
#[inline]
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

// Ported from LightCraft crates/color/src/transfer.rs `linear_to_srgb` (MIT OR Apache-2.0)
#[inline]
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub const LUT_N: usize = 4096;

/// `f` sampled at `LUT_N + 1` points over 0..1, for [`lut`].
pub fn table(f: impl Fn(f32) -> f32) -> Vec<f32> {
    (0..=LUT_N).map(|i| f(i as f32 / LUT_N as f32)).collect()
}

// Ported from LightCraft crates/pipeline/src/finish.rs `encode_srgb` (MIT OR Apache-2.0)
/// Interpolated table lookup (input clamped to 0..1): within 2e-5 of the exact sRGB curve.
#[inline]
pub fn lut(t: &[f32], v: f32) -> f32 {
    let f = v.clamp(0.0, 1.0) * LUT_N as f32;
    let i = (f as usize).min(LUT_N - 1);
    t[i] + (t[i + 1] - t[i]) * (f - i as f32)
}

// ---------------------------------------------------------------- white balance

const D65: (f64, f64) = (0.3127, 0.3290);
/// Linear sRGB → XYZ (D65), the standard matrix LightCraft derives from the sRGB primaries.
const SRGB_TO_XYZ: M3 = [
    [0.412_456_4, 0.357_576_1, 0.180_437_5],
    [0.212_672_9, 0.715_152_2, 0.072_175_0],
    [0.019_333_9, 0.119_192_0, 0.950_304_1],
];
const BRADFORD: M3 = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];
const MIN_K: f64 = 1667.0;
const MAX_K: f64 = 25000.0;

fn xyz((x, y): (f64, f64)) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

// Ported from LightCraft crates/color/src/cct.rs `kim_xy` (MIT OR Apache-2.0)
fn kim_xy(t: f64) -> (f64, f64) {
    let t = t.clamp(MIN_K, MAX_K);
    let (t2, t3) = (t * t, t * t * t);
    let x = if t <= 4000.0 {
        -0.2661239e9 / t3 - 0.2343589e6 / t2 + 0.8776956e3 / t + 0.179910
    } else {
        -3.0258469e9 / t3 + 2.1070379e6 / t2 + 0.2226347e3 / t + 0.240390
    };
    let (x2, x3) = (x * x, x * x * x);
    let y = if t <= 2222.0 {
        -1.1063814 * x3 - 1.34811020 * x2 + 2.18555832 * x - 0.20219683
    } else if t <= 4000.0 {
        -0.9549476 * x3 - 1.37418593 * x2 + 2.09137015 * x - 0.16748867
    } else {
        3.0817580 * x3 - 5.87338670 * x2 + 3.75112997 * x - 0.37001483
    };
    (x, y)
}

fn xy_to_uv((x, y): (f64, f64)) -> (f64, f64) {
    let d = -2.0 * x + 12.0 * y + 3.0;
    (4.0 * x / d, 6.0 * y / d)
}

fn uv_to_xy(u: f64, v: f64) -> (f64, f64) {
    let d = 2.0 * u - 8.0 * v + 4.0;
    (3.0 * u / d, 2.0 * v / d)
}

// Ported from LightCraft crates/color/src/cct.rs `locus_uv` (MIT OR Apache-2.0)
fn locus_uv(t: f64) -> (f64, f64) {
    let t = t.max(1000.0);
    if (MIN_K..=MAX_K).contains(&t) {
        return xy_to_uv(kim_xy(t));
    }
    let (a, b) = if t < MIN_K {
        (MIN_K, MIN_K + 50.0)
    } else {
        (MAX_K - 500.0, MAX_K)
    };
    let (ua, va) = xy_to_uv(kim_xy(a));
    let (ub, vb) = xy_to_uv(kim_xy(b));
    let (ma, mb, m) = (1e6 / a, 1e6 / b, 1e6 / t);
    let s = (m - ma) / (mb - ma);
    (ua + (ub - ua) * s, va + (vb - va) * s)
}

// Ported from LightCraft crates/color/src/cct.rs `temp_tint_to_xy` (at tint 0) (MIT OR Apache-2.0)
fn temp_to_xy(t: f64) -> (f64, f64) {
    let (u, v) = locus_uv(t);
    uv_to_xy(u, v)
}

// Ported from LightCraft crates/color/src/lib.rs `bradford` and crates/color/src/cct.rs
// `wb_matrix` (MIT OR Apache-2.0)
/// Linear sRGB → linear sRGB adaptation of a scene lit by `src` to D65.
fn wb_matrix(src: (f64, f64)) -> M3 {
    let s = apply(&BRADFORD, xyz(src));
    let d = apply(&BRADFORD, xyz(D65));
    let diag = [
        [d[0] / s[0], 0.0, 0.0],
        [0.0, d[1] / s[1], 0.0],
        [0.0, 0.0, d[2] / s[2]],
    ];
    let adapt = mul(&inverse(&BRADFORD), &mul(&diag, &BRADFORD));
    mul(&inverse(&SRGB_TO_XYZ), &mul(&adapt, &SRGB_TO_XYZ))
}

// Ported from LightCraft crates/pipeline/src/local.rs `wb_matrix_for` (rendered-file branch) and
// crates/ui-egui/src/panels/edit/sliders.rs `rel_to_k` (MIT OR Apache-2.0)
/// White balance of a rendered photo on LightCraft's relative scale: `temp` −100..100 is a mired
/// shift around the as-shot 6500 K (positive warmer). Bradford adaptation from the chosen white
/// to D65, normalised so neutral luminance is kept.
pub fn white_balance(temp: f64) -> [[f32; 3]; 3] {
    let kelvin = 1e6 / (1e6 / 6500.0 - temp * 0.8);
    let m = mul(
        &wb_matrix(temp_to_xy(kelvin)),
        &inverse(&wb_matrix(temp_to_xy(6500.0))),
    );
    let g = apply(&m, [1.0; 3]);
    let y = g[0] * SRGB_TO_XYZ[1][0] + g[1] * SRGB_TO_XYZ[1][1] + g[2] * SRGB_TO_XYZ[1][2];
    m.map(|r| r.map(|v| (v / y) as f32))
}

// ---------------------------------------------------------------- tone

#[inline]
pub fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

// Ported from LightCraft crates/film/src/lib.rs `Converter::new` (brightness as a gamma) (MIT OR Apache-2.0)
/// Brightness −1..1 as the exponent of a mid-tone gamma on linear values: black and white stay
/// put, so nothing clips (+1 is an exponent of 0.5, −1 of 2).
pub fn brightness_gamma(brightness: f32) -> f32 {
    (-brightness).exp2()
}

// Ported from LightCraft crates/pipeline/src/tone.rs `ToneMap::display` (contrast term) (MIT OR Apache-2.0)
/// Contrast −1..1 as LightCraft's display S-curve on linear luminance `y` (0..1): applied in a
/// gamma-2.2 domain and anchored at black, middle grey (0.18) and white.
pub fn contrast_curve(contrast: f32, y: f32) -> f32 {
    let m = 0.18f32.powf(1.0 / 2.2);
    let mut p = y.clamp(0.0, 1.0).powf(1.0 / 2.2);
    p += contrast * 0.35 * (p - m) * (1.0 - (2.0 * p - 1.0).powi(2));
    p.clamp(0.0, 1.0).powf(2.2)
}

// Ported from LightCraft crates/pipeline/src/finish.rs `gamut_map_to` (MIT OR Apache-2.0)
/// Desaturate linear `r` towards its luminance until every channel is in 0..1, so an
/// out-of-range colour keeps its hue instead of clipping channel by channel.
#[inline]
pub fn gamut_map(r: [f32; 3]) -> [f32; 3] {
    let yy = luminance(r).clamp(0.0, 1.0);
    let mut t = 1.0f32;
    for c in r {
        if c < 0.0 {
            t = t.min(yy / (yy - c).max(1e-9));
        } else if c > 1.0 {
            t = t.min((1.0 - yy) / (c - yy).max(1e-9));
        }
    }
    if t < 1.0 {
        r.map(|c| yy + (c - yy) * t)
    } else {
        r
    }
}

// ---------------------------------------------------------------- colour (OkLab)

// Björn Ottosson's OkLab matrices for linear sRGB (LightCraft derives its Rec.2020 ones from
// the same definition, crates/color/src/perceptual.rs).
const TO_LMS: [[f32; 3]; 3] = [
    [0.412_221_46, 0.536_332_55, 0.051_445_995],
    [0.211_903_5, 0.680_699_5, 0.107_396_96],
    [0.088_302_46, 0.281_718_85, 0.629_978_7],
];
const FROM_LMS: [[f32; 3]; 3] = [
    [4.076_741_7, -3.307_711_6, 0.230_969_94],
    [-1.268_438, 2.609_757_4, -0.341_319_38],
    [-0.004_196_086_3, -0.703_418_6, 1.707_614_7],
];
const TO_LAB: [[f32; 3]; 3] = [
    [0.210_454_26, 0.793_617_8, -0.004_072_047],
    [1.977_998_5, -2.428_592_2, 0.450_593_7],
    [0.025_904_037, 0.782_771_77, -0.808_675_77],
];
const FROM_LAB: [[f32; 3]; 3] = [
    [1.0, 0.396_337_78, 0.215_803_76],
    [1.0, -0.105_561_346, -0.063_854_17],
    [1.0, -0.089_484_18, -1.291_485_5],
];

/// Cube root by a bit-level first guess and two Halley steps (relative error < 1e-6 in the
/// 0..16 range used here), about twice as fast as `f32::cbrt` on MSVC. Our own helper.
#[inline]
fn cbrt(x: f32) -> f32 {
    if x <= 0.0 {
        return -cbrt_pos(-x);
    }
    cbrt_pos(x)
}

#[inline]
fn cbrt_pos(x: f32) -> f32 {
    if x == 0.0 {
        return 0.0;
    }
    let mut y = f32::from_bits(x.to_bits() / 3 + 709_921_077);
    for _ in 0..2 {
        let y3 = y * y * y;
        y *= (y3 + 2.0 * x) / (2.0 * y3 + x);
    }
    y
}

// Ported from LightCraft crates/color/src/perceptual.rs `oklab_from_2020` (MIT OR Apache-2.0)
#[inline]
fn oklab(rgb: [f32; 3]) -> [f32; 3] {
    apply_f32(&TO_LAB, apply_f32(&TO_LMS, rgb).map(cbrt))
}

// Ported from LightCraft crates/color/src/perceptual.rs `oklab_to_2020` (MIT OR Apache-2.0)
#[inline]
fn from_oklab(lab: [f32; 3]) -> [f32; 3] {
    apply_f32(&FROM_LMS, apply_f32(&FROM_LAB, lab).map(|v| v * v * v))
}

// Ported from LightCraft crates/pipeline/src/colorops.rs `ColorOps::apply` (saturation) (MIT OR Apache-2.0)
/// Saturation −1..1 scales OkLab chroma: lightness and hue stay, −1 is grey.
#[inline]
pub fn saturate(rgb: [f32; 3], saturation: f32) -> [f32; 3] {
    let mut lab = oklab(rgb);
    let k = (1.0 + saturation).max(0.0);
    lab[1] *= k;
    lab[2] *= k;
    from_oklab(lab)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_white_balance_is_identity_and_warm_is_redder() {
        let m = white_balance(0.0);
        for (i, row) in m.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                assert!((v - f32::from(i == j)).abs() < 1e-5, "{m:?}");
            }
        }
        let warm = apply_f32(&white_balance(50.0), [0.5; 3]);
        assert!(warm[0] > warm[2]);
    }

    #[test]
    fn tone_curves_keep_black_grey_and_white() {
        for c in [-1.0, 0.5, 1.0] {
            assert_eq!(contrast_curve(c, 0.0), 0.0);
            assert!((contrast_curve(c, 1.0) - 1.0).abs() < 1e-6);
            assert!((contrast_curve(c, 0.18) - 0.18).abs() < 1e-4);
        }
        assert!(contrast_curve(0.5, 0.6) > 0.6 && contrast_curve(0.5, 0.05) < 0.05);
        let g = brightness_gamma(0.5);
        assert!(g < 1.0 && 1f32.powf(g) == 1.0 && 0.18f32.powf(g) > 0.18);
    }

    #[test]
    fn fast_cbrt_matches_std() {
        for i in 0..10_000 {
            let x = i as f32 / 1000.0 - 1.0;
            let (a, b) = (cbrt(x), x.cbrt());
            assert!((a - b).abs() <= 2e-6 * b.abs().max(1e-3), "{x}: {a} vs {b}");
        }
        assert_eq!(cbrt(0.0), 0.0);
    }

    #[test]
    fn oklab_round_trips_and_saturation_keeps_hue() {
        for c in [[0.2, 0.5, 0.9], [1.0, 0.0, 0.0], [0.01, 0.02, 0.03]] {
            let back = from_oklab(oklab(c));
            assert!(
                c.iter().zip(back).all(|(a, b)| (a - b).abs() < 1e-4),
                "{back:?}"
            );
        }
        let (a, b) = (
            oklab([0.6, 0.3, 0.1]),
            oklab(saturate([0.6, 0.3, 0.1], -0.5)),
        );
        assert!((a[2].atan2(a[1]) - b[2].atan2(b[1])).abs() < 1e-3 && (a[0] - b[0]).abs() < 1e-4);
    }
}
