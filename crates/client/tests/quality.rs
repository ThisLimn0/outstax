//! Print quality report: `cargo test --release -p outstax-client --test quality -- --ignored --nocapture`
//!
//! Runs the real pipeline (`prepare_with_options`, default processing, Fill) for each test image
//! and film format at the printer's byte budget, and compares the decoded JPEG with an independent
//! reference: a linear-light f64 Lanczos3 downscale of the same crop, encoded to sRGB without
//! dither. Extra photos come from `OUTSTAX_QUALITY_IMAGES` (paths separated by `;`). Never commit
//! photos.

use image::{DynamicImage, Rgb, RgbImage};
use outstax_client::imaging::{self, Crop, Fit, Processing};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    f64::consts::PI,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    time::Instant,
};

/// Heap high-water mark, so the report can show the pipeline's peak extra memory.
struct Counting;
static NOW: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            PEAK.fetch_max(NOW.fetch_add(l.size(), Relaxed) + l.size(), Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        NOW.fetch_sub(l.size(), Relaxed);
    }
}
#[global_allocator]
static ALLOC: Counting = Counting;

const FORMATS: [(&str, u32, u32, usize); 3] = [
    ("mini", 600, 800, 153_600),
    ("square", 800, 800, 107_520),
    ("wide", 1260, 840, 337_920),
];

fn hash(x: u32, y: u32, c: u32) -> u32 {
    let mut h =
        x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}
fn noise(x: u32, y: u32, c: u32) -> f64 {
    hash(x, y, c) as f64 / u32::MAX as f64 - 0.5
}
fn px(v: [f64; 3]) -> Rgb<u8> {
    Rgb(v.map(|v| v.round().clamp(0.0, 255.0) as u8))
}

// 1.5707 is a near-vertical line on purpose, not a stand-in for pi / 2.
#[allow(clippy::approx_constant)]
fn generated() -> Vec<(String, DynamicImage)> {
    let (w, h) = (3000, 2000);
    let sky = RgbImage::from_fn(w, h, |x, y| {
        let t = (y as f64 / h as f64).powf(1.4);
        let s = 0.03 * (x as f64 / w as f64);
        let top = [28.0, 78.0, 168.0];
        let horizon = [196.0, 218.0, 238.0];
        px(std::array::from_fn(|i| {
            top[i] + (horizon[i] - top[i]) * (t + s)
        }))
    });
    let texture = RgbImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f64 - 1500.0, y as f64 - 1000.0);
        let zone = (PI * (fx * fx + fy * fy) / 3600.0).cos();
        let weave = ((x as f64 * 1.3).sin() * (y as f64 * 0.9).cos()) * 0.5 + 0.5;
        px([
            128.0 + 110.0 * noise(x, y, 0) * 1.8,
            128.0 + 100.0 * zone,
            40.0 + 180.0 * weave,
        ])
    });
    let colours: [[f64; 3]; 8] = [
        [255.0, 0.0, 0.0],
        [0.0, 255.0, 0.0],
        [0.0, 0.0, 255.0],
        [0.0, 255.0, 255.0],
        [255.0, 0.0, 255.0],
        [255.0, 255.0, 0.0],
        [255.0, 128.0, 0.0],
        [20.0, 20.0, 20.0],
    ];
    let edges = RgbImage::from_fn(w, h, |x, y| {
        let diagonal = (x + 2 * y) / 131;
        let tile = (x / 97 + 3 * (y / 89) + diagonal) as usize;
        px(colours[tile % colours.len()])
    });
    let lines = RgbImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f64, y as f64);
        let mut ink = false;
        // Straight lines of width 1 to 3 at several angles, in the top half.
        if y < h / 2 {
            for (k, angle) in [0.0f64, 0.08, 0.6, 1.1, 1.5707].iter().enumerate() {
                let (s, c) = angle.sin_cos();
                let d = (fx * s - fy * c).rem_euclid(53.0 + k as f64 * 7.0);
                ink |= d < 1.0 + (k % 3) as f64;
            }
        } else {
            // Glyph-like strokes: 2 px strokes in 24x32 cells chosen by a hash.
            let (cx, cy) = (x / 24, y / 32);
            let (u, v) = (x % 24, y % 32);
            let bits = hash(cx, cy, 7);
            ink = (bits & 1 != 0 && (4..6).contains(&u) && (4..28).contains(&v))
                || (bits & 2 != 0 && (16..18).contains(&u) && (4..28).contains(&v))
                || (bits & 4 != 0 && (4..18).contains(&u) && (4..6).contains(&v))
                || (bits & 8 != 0 && (4..18).contains(&u) && (15..17).contains(&v))
                || (bits & 16 != 0 && (4..18).contains(&u) && (26..28).contains(&v));
        }
        px(if ink { [16.0; 3] } else { [250.0; 3] })
    });
    let skin = RgbImage::from_fn(w, h, |x, y| {
        let (fx, fy) = (x as f64 / w as f64, y as f64 / h as f64);
        let shade = 0.82 + 0.18 * (fx * 5.0).sin() * (fy * 3.0 + 0.5).cos();
        let n = noise(x / 2, y / 2, 3) * 10.0;
        px([
            226.0 * shade + n,
            176.0 * shade * shade + n,
            146.0 * shade * shade * shade + n,
        ])
    });
    [
        ("sky gradient", sky),
        ("fine texture", texture),
        ("colour edges", edges),
        ("lines and text", lines),
        ("skin tones", skin),
    ]
    .into_iter()
    .map(|(n, i)| (n.to_owned(), DynamicImage::ImageRgb8(i)))
    .collect()
}

fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn linear_to_srgb(v: f64) -> f64 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
fn lanczos3(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else if x.abs() < 3.0 {
        let p = PI * x;
        3.0 * p.sin() * (p / 3.0).sin() / (p * p)
    } else {
        0.0
    }
}
/// For each output sample: the first source index and its normalised Lanczos3 weights. The output
/// covers the source span `start..start + len` (continuous coordinates, pixel i spans i..i+1).
fn weights(src: usize, start: f64, len: f64, out: usize) -> Vec<(usize, Vec<f64>)> {
    let scale = len / out as f64;
    let s = scale.max(1.0);
    (0..out)
        .map(|i| {
            let c = start + (i as f64 + 0.5) * scale;
            let lo = (c - 3.0 * s).floor().max(0.0) as usize;
            let hi = ((c + 3.0 * s).ceil() as usize).min(src);
            let mut w: Vec<f64> = (lo..hi)
                .map(|j| lanczos3((j as f64 + 0.5 - c) / s))
                .collect();
            let sum: f64 = w.iter().sum();
            w.iter_mut().for_each(|v| *v /= sum);
            (lo, w)
        })
        .collect()
}

/// Independent high-precision Fill: same crop rectangle as the pipeline, linear f64 Lanczos3.
fn reference(src: &RgbImage, w: u32, h: u32) -> RgbImage {
    let (sw, sh) = src.dimensions();
    let (_, [l, t, cw, ch]) = Crop::default().fit(sw, sh, w as f64 / h as f64);
    let lut: Vec<f64> = (0..256).map(|v| srgb_to_linear(v as f64 / 255.0)).collect();
    let xs = weights(sw as usize, l, cw, w as usize);
    let ys = weights(sh as usize, t, ch, h as usize);
    let first = ys[0].0;
    let last = ys.iter().map(|(lo, w)| lo + w.len()).max().unwrap();
    let raw = src.as_raw();
    // Horizontal pass over the rows the vertical pass needs only.
    let mut tmp = vec![0.0f64; (last - first) * w as usize * 3];
    for y in first..last {
        let row = &raw[y * sw as usize * 3..(y + 1) * sw as usize * 3];
        for (x, (lo, wt)) in xs.iter().enumerate() {
            let mut acc = [0.0; 3];
            for (k, wv) in wt.iter().enumerate() {
                for c in 0..3 {
                    acc[c] += wv * lut[row[(lo + k) * 3 + c] as usize];
                }
            }
            tmp[((y - first) * w as usize + x) * 3..][..3].copy_from_slice(&acc);
        }
    }
    RgbImage::from_fn(w, h, |x, y| {
        let (lo, wt) = &ys[y as usize];
        let mut acc = [0.0; 3];
        for (k, wv) in wt.iter().enumerate() {
            for c in 0..3 {
                acc[c] += wv * tmp[((lo + k - first) * w as usize + x as usize) * 3 + c];
            }
        }
        Rgb(acc.map(|v| (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8))
    })
}

fn psnr(a: &RgbImage, b: &RgbImage) -> f64 {
    let n = a.as_raw().len() as f64;
    let mse = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(&x, &y)| (x as f64 - y as f64).powi(2))
        .sum::<f64>()
        / n;
    10.0 * (255.0 * 255.0 / mse.max(1e-12)).log10()
}

/// Standard SSIM (Wang et al. 2004) on BT.601 luma: 11x11 Gaussian window, sigma 1.5,
/// K1 0.01, K2 0.03, averaged over the windows that fit inside the image.
fn ssim(a: &RgbImage, b: &RgbImage) -> f64 {
    let (w, h) = (a.width() as usize, a.height() as usize);
    let luma = |i: &RgbImage| -> Vec<f64> {
        i.pixels()
            .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
            .collect()
    };
    let (x, y) = (luma(a), luma(b));
    let g: Vec<f64> = {
        let g: Vec<f64> = (-5..=5i32)
            .map(|i| (-(i * i) as f64 / (2.0 * 1.5 * 1.5)).exp())
            .collect();
        let s: f64 = g.iter().sum();
        g.into_iter().map(|v| v / s).collect()
    };
    // Valid-region separable blur: output (w-10)x(h-10).
    let blur = |f: &dyn Fn(usize) -> f64| -> Vec<f64> {
        let (ow, oh) = (w - 10, h - 10);
        let mut tmp = vec![0.0; ow * h];
        for r in 0..h {
            for c in 0..ow {
                tmp[r * ow + c] = (0..11).map(|k| g[k] * f(r * w + c + k)).sum();
            }
        }
        let mut out = vec![0.0; ow * oh];
        for r in 0..oh {
            for c in 0..ow {
                out[r * ow + c] = (0..11).map(|k| g[k] * tmp[(r + k) * ow + c]).sum();
            }
        }
        out
    };
    let mx = blur(&|i| x[i]);
    let my = blur(&|i| y[i]);
    let mxx = blur(&|i| x[i] * x[i]);
    let myy = blur(&|i| y[i] * y[i]);
    let mxy = blur(&|i| x[i] * y[i]);
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let sum: f64 = (0..mx.len())
        .map(|i| {
            let (ux, uy) = (mx[i], my[i]);
            let (vx, vy, cxy) = (mxx[i] - ux * ux, myy[i] - uy * uy, mxy[i] - ux * uy);
            ((2.0 * ux * uy + c1) * (2.0 * cxy + c2)) / ((ux * ux + uy * uy + c1) * (vx + vy + c2))
        })
        .sum();
    sum / mx.len() as f64
}

/// Banding along the columns of a smooth vertical gradient: the mean longest run of identical
/// green values per column, and the largest step between neighbours.
fn banding(i: &RgbImage) -> (f64, u8) {
    let (w, h) = i.dimensions();
    let (mut runs, mut step) = (0usize, 0u8);
    for x in 0..w {
        let (mut best, mut run) = (1, 1);
        for y in 1..h {
            let (a, b) = (i.get_pixel(x, y - 1)[1], i.get_pixel(x, y)[1]);
            step = step.max(a.abs_diff(b));
            run = if a == b { run + 1 } else { 1 };
            best = best.max(run);
        }
        runs += best;
    }
    (runs as f64 / w as f64, step)
}

fn report(name: &str, image: &DynamicImage) {
    let source = image.to_rgb8();
    for (format, w, h, budget) in FORMATS {
        let input = image.clone();
        let base = NOW.load(Relaxed);
        PEAK.store(base, Relaxed);
        let start = Instant::now();
        let p =
            imaging::prepare_with_options(input, w, h, budget, Fit::Crop, Processing::default())
                .unwrap();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        let peak = (PEAK.load(Relaxed) - base) as f64 / 1048576.0;
        assert!(p.jpeg.len() <= budget);
        if format == "mini" {
            outstax_protocol::jpeg::check_mini1(&p.jpeg).unwrap();
        }
        let out = image::load_from_memory(&p.jpeg).unwrap().to_rgb8();
        let want = reference(&source, w, h);
        let band = if name == "sky gradient" {
            let (r, s) = banding(&out);
            let (rr, rs) = banding(&want);
            format!("  run {r:.1} step {s} (reference run {rr:.1} step {rs})")
        } else {
            String::new()
        };
        println!(
            "{name:<16} {format:<6} q{:<3} {:>7} B  PSNR {:>6.2} dB  SSIM {:.5}  {ms:>7.1} ms  peak {peak:>6.1} MB{band}",
            p.quality,
            p.jpeg.len(),
            psnr(&out, &want),
            ssim(&out, &want),
        );
    }
}

#[test]
#[ignore]
fn quality_report() {
    println!("image            format quality  bytes      PSNR          SSIM        time       peak extra heap");
    for (name, image) in generated() {
        report(&name, &image);
    }
    let paths = std::env::var("OUTSTAX_QUALITY_IMAGES").unwrap_or_default();
    for path in paths.split(';').filter(|p| !p.trim().is_empty()) {
        let path = std::path::Path::new(path.trim());
        let start = Instant::now();
        let image = imaging::load(path).unwrap();
        let name = path.file_name().unwrap().to_string_lossy();
        println!(
            "{name}: {}x{}, load {:.0} ms",
            image.width(),
            image.height(),
            start.elapsed().as_secs_f64() * 1000.0
        );
        report(&format!("photo {}", &name[..name.len().min(10)]), &image);
        // Desktop preview: a 1600 px copy at the mini budget (the old preview used 8 MB).
        let small = image.thumbnail(1600, 1600);
        for budget in [153_600, 8 << 20] {
            let start = Instant::now();
            let p = imaging::prepare_with_options(
                small.clone(),
                600,
                800,
                budget,
                Fit::Crop,
                Processing::default(),
            )
            .unwrap();
            println!(
                "  preview at {budget} B: q{} {} B in {:.1} ms",
                p.quality,
                p.jpeg.len(),
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}
