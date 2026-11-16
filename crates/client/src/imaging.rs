use anyhow::{bail, Context, Result};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, ImageDecoder,
    ImageReader, Rgb, RgbImage,
};
use log::{info, warn};
use std::{io::Cursor, path::Path};

#[derive(Debug, Clone, Copy)]
pub enum Fit {
    Crop,
    Contain,
}
pub struct PreparedImage {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub quality: u8,
}
pub fn load_memory(bytes: &[u8]) -> Result<(Vec<u8>, [usize; 2])> {
    let image = image::load_from_memory(bytes)?.to_rgb8();
    let (w, h) = image.dimensions();
    Ok((image.into_raw(), [w as usize, h as usize]))
}

/// Decode with allocation limits, convert an embedded colour profile to sRGB, apply EXIF
/// orientation. Alpha stays: [`prepare_with_options`] composites it on white.
pub fn load(path: &Path) -> Result<DynamicImage> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut decoder = reader
        .into_decoder()
        .context("unsupported image format. Use JPEG, PNG or WebP")?;
    let orientation = decoder.orientation()?;
    let icc = decoder.icc_profile().unwrap_or_else(|e| {
        warn!("ignoring unreadable colour profile: {e}");
        None
    });
    let mut image = DynamicImage::from_decoder(decoder)?;
    if let Some(icc) = icc {
        if let Err(e) = to_srgb(&mut image, &icc) {
            warn!("colour profile not applied: {e}");
        }
    }
    image.apply_orientation(orientation);
    Ok(image)
}

/// Converts pixels tagged with a non-sRGB ICC profile to sRGB, in place, with moxcms's perceptual
/// intent (the one both matrix and LUT profiles always support). A profile that maps every probed
/// colour to within 1 of itself counts as sRGB and leaves the pixels untouched.
fn to_srgb(image: &mut DynamicImage, icc: &[u8]) -> Result<(), moxcms::CmsError> {
    use moxcms::{ColorProfile, Layout, ProfileText, TransformExecutor, TransformOptions};
    fn rows<T: Copy + Default>(
        data: &mut [T],
        row: usize,
        t: &(impl TransformExecutor<T> + ?Sized),
    ) -> Result<(), moxcms::CmsError> {
        let mut out = vec![T::default(); row];
        for chunk in data.chunks_mut(row) {
            t.transform(chunk, &mut out)?;
            chunk.copy_from_slice(&out);
        }
        Ok(())
    }
    let layout = match image {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgb16(_) => Layout::Rgb,
        DynamicImage::ImageRgba8(_) | DynamicImage::ImageRgba16(_) => Layout::Rgba,
        _ => return Ok(()), // ponytail: grey images keep their grey profile unconverted.
    };
    let profile = ColorProfile::new_from_slice(icc)?;
    let srgb = ColorProfile::new_srgb();
    let options = TransformOptions::default();
    let probe = profile.create_transform_8bit(Layout::Rgb, &srgb, Layout::Rgb, options)?;
    let grid: Vec<u8> = (0..18u32.pow(3) * 3)
        .map(|i| (i / 3 / 18u32.pow(i % 3) % 18 * 15) as u8)
        .collect();
    let mut mapped = vec![0; grid.len()];
    probe.transform(&grid, &mut mapped)?;
    if grid.iter().zip(&mapped).all(|(a, b)| a.abs_diff(*b) <= 1) {
        return Ok(());
    }
    let row = image.width() as usize * if layout == Layout::Rgb { 3 } else { 4 };
    match image {
        DynamicImage::ImageRgb8(i) => rows(i, row, &*probe)?,
        DynamicImage::ImageRgba8(i) => rows(
            i,
            row,
            &*profile.create_transform_8bit(layout, &srgb, layout, options)?,
        )?,
        DynamicImage::ImageRgb16(i) => rows(
            i,
            row,
            &*profile.create_transform_16bit(layout, &srgb, layout, options)?,
        )?,
        DynamicImage::ImageRgba16(i) => rows(
            i,
            row,
            &*profile.create_transform_16bit(layout, &srgb, layout, options)?,
        )?,
        _ => {}
    }
    let name = match &profile.description {
        Some(ProfileText::PlainString(s)) => s.clone(),
        Some(ProfileText::Localizable(l)) => l.first().map(|s| s.value.clone()).unwrap_or_default(),
        Some(ProfileText::Description(d)) => d.ascii_string.clone(),
        None => String::new(),
    };
    info!("converted the embedded colour profile {name:?} to sRGB");
    Ok(())
}

/// Slider adjustments of our own (not vendor processing), applied at print resolution after the
/// resize. Each value is -100..=100; zero is neutral, and all-zero returns the image unchanged.
/// The colour math is ported from LightCraft, see [`crate::lightcraft`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Adjustments {
    pub brightness: i8,
    pub contrast: i8,
    pub saturation: i8,
    /// Positive is warmer (LightCraft's relative scale: a mired shift around 6500 K).
    pub temperature: i8,
    /// Clockwise quarter turns.
    pub quarter_turns: u8,
    /// Fill-mode framing. [`adjust`] ignores it, [`prepare_with_options`] applies it.
    pub crop: Crop,
}

/// Fill-mode framing of the rotated image: the crop centre as a fraction of the image width and
/// height, and a zoom from 1 to [`Crop::MAX_ZOOM`]. The default (centred, 1x) is the plain
/// centred fill. Fit mode ignores it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}
impl Default for Crop {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 0.5,
            zoom: 1.0,
        }
    }
}
impl Crop {
    pub const MAX_ZOOM: f32 = 4.0;
    /// The only place a crop is limited: clamps zoom and centre so the crop stays inside a `w`x`h`
    /// image at the output `aspect` (width / height). Returns the clamped crop and its source
    /// rectangle in pixels, `[left, top, width, height]`.
    pub fn fit(self, w: u32, h: u32, aspect: f64) -> (Crop, [f64; 4]) {
        let (w, h) = (w as f64, h as f64);
        let zoom = self.zoom.clamp(1.0, Self::MAX_ZOOM);
        let (cw, ch) = if w / h > aspect {
            (h * aspect, h)
        } else {
            (w, w / aspect)
        };
        let (cw, ch) = (cw / zoom as f64, ch / zoom as f64);
        let cx = (self.x as f64 * w).clamp(cw / 2.0, w - cw / 2.0);
        let cy = (self.y as f64 * h).clamp(ch / 2.0, h - ch / 2.0);
        let crop = Crop {
            x: (cx / w) as f32,
            y: (cy / h) as f32,
            zoom,
        };
        (crop, [cx - cw / 2.0, cy - ch / 2.0, cw, ch])
    }
}

/// Clockwise quarter turns.
fn rotate(image: DynamicImage, quarter_turns: u8) -> DynamicImage {
    match quarter_turns % 4 {
        1 => image.rotate90(),
        2 => image.rotate180(),
        3 => image.rotate270(),
        _ => image,
    }
}

/// The colour and tone stages on one linear-light pixel, in LightCraft's stage order
/// (crates/pipeline/src/lib.rs):
/// 1. scene linear: Color temperature (Bradford white balance, neutral luminance kept);
/// 2. tone: Brightness (a mid-tone gamma that keeps black and white), then Contrast (an S-curve on
///    luminance anchored at black, grey and white, colour ratios kept);
/// 3. colour: Saturation (OkLab chroma, so lightness and hue stay);
/// 4. display: gamut map (desaturate out-of-range colours towards their luminance).
///
/// `None` when every slider is zero: the identity, with no gamut map either.
fn tone(a: Adjustments) -> Option<impl Fn([f32; 3]) -> [f32; 3] + Sync> {
    use crate::lightcraft as lc;
    if [a.brightness, a.contrast, a.saturation, a.temperature] == [0; 4] {
        return None;
    }
    let [brightness, contrast, saturation] =
        [a.brightness, a.contrast, a.saturation].map(|v| v as f32 / 100.0);
    let gamma = lc::brightness_gamma(brightness);
    let curve = lc::table(|y| lc::contrast_curve(contrast, y));
    let wb = (a.temperature != 0).then(|| lc::white_balance(a.temperature as f64));
    Some(move |mut c: [f32; 3]| {
        if let Some(m) = &wb {
            let v = c;
            c = std::array::from_fn(|i| {
                (m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]).max(0.0)
            });
        }
        if brightness != 0.0 {
            c = c.map(|v| v.powf(gamma));
        }
        if contrast != 0.0 {
            let y = lc::luminance(c);
            if y > 1e-9 {
                let k = lc::lut(&curve, y) / y;
                c = c.map(|v| v * k);
            }
        }
        if saturation != 0.0 {
            c = lc::saturate(c, saturation);
        }
        lc::gamut_map(c)
    })
}

/// Rotate, then [`tone`] each 8-bit pixel (sRGB decode, stages, sRGB encode). All-zero settings
/// return the input unchanged. [`prepare_with_options`] runs the same stages after resizing.
pub fn adjust(image: DynamicImage, a: Adjustments) -> DynamicImage {
    use crate::lightcraft as lc;
    let image = rotate(image, a.quarter_turns);
    let Some(tone) = tone(a) else {
        return image;
    };
    let decode: [f32; 256] = std::array::from_fn(|i| lc::srgb_to_linear(i as f32 / 255.0));
    let encode = lc::table(lc::linear_to_srgb);
    let mut rgba = image.into_rgba8();
    let row = rgba.width() as usize * 4;
    par_rows(&mut rgba, row, |_, row| {
        for p in row.as_chunks_mut::<4>().0 {
            let c = tone([0, 1, 2].map(|i| decode[p[i] as usize]));
            for (i, v) in c.into_iter().enumerate() {
                p[i] = (lc::lut(&encode, v) * 255.0 + 0.5) as u8;
            }
        }
    });
    DynamicImage::ImageRgba8(rgba)
}

/// Runs `f(row number, row)` over `data` cut into rows of `row` items, in bands on scoped std
/// threads (no rayon).
fn par_rows<T: Send>(data: &mut [T], row: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    if data.is_empty() || row == 0 {
        return;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    let band = (data.len() / row).div_ceil(threads).max(1);
    let f = &f;
    std::thread::scope(|s| {
        for (b, chunk) in data.chunks_mut(band * row).enumerate() {
            s.spawn(move || {
                for (i, r) in chunk.chunks_mut(row).enumerate() {
                    f(b * band + i, r);
                }
            });
        }
    });
}

/// The firmware's JPEG size limit for a film size: mini Link, Link 2, Link 3 and Link+ take
/// 153,600 bytes and Link WIDE 337,920. SQUARE's is unknown, so it and any other size get the
/// conservative 107,520 (about what the vendor app sends to SQUARE).
pub fn default_max_bytes(width: u32, height: u32) -> usize {
    match (width, height) {
        (600, 800) => 153_600,
        (1260, 840) => 337_920,
        _ => 107_520,
    }
}

pub fn prepare(
    image: DynamicImage,
    width: u32,
    height: u32,
    max_bytes: usize,
    fit: Fit,
) -> Result<PreparedImage> {
    prepare_with_lut(image, width, height, max_bytes, fit, None)
}

/// Optional exact 24-bit color lookup table: 2^24 four-byte [reserved,R,G,B] records.
/// The index is (R<<16)|(G<<8)|B. Vendor assets are not included in this project.
pub fn prepare_with_lut(
    image: DynamicImage,
    width: u32,
    height: u32,
    max_bytes: usize,
    fit: Fit,
    lut: Option<&[u8]>,
) -> Result<PreparedImage> {
    prepare_job(image, width, height, max_bytes, fit, lut, false)
}

/// Design Print composes at 2W×2H, aligns by +0.5 degrees, then halves width.
/// Geometry recovered from Mini Link 7.0.6; interpolation/JPEG are independent implementations.
pub fn prepare_job(
    image: DynamicImage,
    width: u32,
    height: u32,
    max_bytes: usize,
    fit: Fit,
    lut: Option<&[u8]>,
    design: bool,
) -> Result<PreparedImage> {
    prepare_with_options(
        image,
        width,
        height,
        max_bytes,
        fit,
        Processing {
            lut,
            design,
            ..Default::default()
        },
    )
}
#[derive(Default)]
pub struct Processing<'a> {
    pub lut: Option<&'a [u8]>,
    pub design: bool,
    pub unsharp: Option<crate::sharpness::Unsharp>,
    /// The sliders, rotation and Fill-mode crop.
    pub adjust: Adjustments,
}
/// Rotation -> linear-light resize to the composition (Fill crop or Fit) -> adjustments at print
/// resolution -> optional linear-light USM -> optional Rich LUT -> Design warp -> JPEG. This is
/// an independent pipeline. FDIA enhancement is separate.
pub fn prepare_with_options(
    image: DynamicImage,
    width: u32,
    height: u32,
    max_bytes: usize,
    fit: Fit,
    processing: Processing<'_>,
) -> Result<PreparedImage> {
    let Processing {
        lut,
        design,
        unsharp,
        adjust,
    } = processing;
    if width == 0 || height == 0 || width > 8192 || height > 8192 || max_bytes == 0 {
        bail!("invalid output image limits");
    }
    // The app sharpens only in the Design Print path, on the doubled composition
    // (the vendor's native code). At plain print size the sigma would act ~2x stronger.
    if unsharp.is_some() && !design {
        bail!("unsharp is only defined for Design Print (--design)");
    }
    let output_width = width;
    let output_height = if design {
        height.checked_mul(2).context("height overflow")?
    } else {
        height
    };
    let (width, height) = if design {
        if width > 4096 || height > 4096 {
            bail!("design composition exceeds limits");
        }
        (width * 2, height * 2)
    } else {
        (width, height)
    };
    let mut rgb = compose(
        rotate(image, adjust.quarter_turns),
        width,
        height,
        fit,
        adjust,
    );
    if let Some(settings) = unsharp {
        rgb = crate::sharpness::apply(&rgb, settings)?;
    }
    if let Some(lut) = lut {
        if lut.len() != 4 * 256 * 256 * 256 {
            bail!("LUT must contain exactly 67108864 bytes");
        }
        for p in rgb.pixels_mut() {
            let i = ((p[0] as usize) << 16) | ((p[1] as usize) << 8) | p[2] as usize;
            p.0.copy_from_slice(&lut[4 * i + 1..4 * i + 4]);
        }
    }
    if design {
        rgb = image::imageops::resize(
            &rotate_cubic(&rgb, 0.5),
            output_width,
            output_height,
            FilterType::Triangle,
        );
    }
    let (jpeg, quality) = encode_within(&rgb, max_bytes)?;
    Ok(PreparedImage {
        jpeg,
        width: output_width,
        height: output_height,
        quality,
    })
}

/// The highest JPEG quality (1..=100) whose file fits `max_bytes`. Size is only nearly monotone
/// in quality, so the binary search is followed by a look at the two qualities above it.
fn encode_within(rgb: &RgbImage, max_bytes: usize) -> Result<(Vec<u8>, u8)> {
    let encode = |quality| {
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality).encode_image(rgb)?;
        Ok::<_, anyhow::Error>((jpeg.len() <= max_bytes).then_some(jpeg))
    };
    let (mut lo, mut hi, mut best) = (1u8, 100u8, None);
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        match encode(mid)? {
            Some(jpeg) => {
                best = Some((jpeg, mid));
                lo = mid + 1;
            }
            None => hi = mid - 1,
        }
    }
    let Some((mut jpeg, mut quality)) = best else {
        bail!("image cannot fit the printer's byte limit at minimum JPEG quality")
    };
    // `quality + 1` failed in the search (or is 101), so only `quality + 2` can still surprise.
    if quality <= 98 {
        if let Some(better) = encode(quality + 2)? {
            (jpeg, quality) = (better, quality + 2);
        }
    }
    Ok((jpeg, quality))
}

/// A source sample type: its largest value, which is also its decode table's last index.
trait Sample: Copy + Send + Sync {
    const MAX: usize;
    fn index(self) -> usize;
}
impl Sample for u8 {
    const MAX: usize = 255;
    fn index(self) -> usize {
        self as usize
    }
}
impl Sample for u16 {
    const MAX: usize = 65535;
    fn index(self) -> usize {
        self as usize
    }
}

/// The composition: `width`x`height` sRGB on white. Fit scales the whole image to fit inside,
/// centred. Fill covers the frame with `a.crop`'s exact source rectangle. Resizing runs in linear
/// light, then the adjustments, then alpha over white, rounded to 8 bits.
fn compose(image: DynamicImage, width: u32, height: u32, fit: Fit, a: Adjustments) -> RgbImage {
    let (iw, ih) = image.dimensions();
    let (rect, w, h) = match fit {
        Fit::Contain => {
            let r = (width as f64 / iw as f64).min(height as f64 / ih as f64);
            let size = |v: u32, max: u32| ((v as f64 * r).round() as u32).clamp(1, max);
            let full = [0.0, 0.0, iw as f64, ih as f64];
            (full, size(iw, width), size(ih, height))
        }
        Fit::Crop => (
            a.crop.fit(iw, ih, width as f64 / height as f64).1,
            width,
            height,
        ),
    };
    let f = Frame {
        src: (iw as usize, ih as usize),
        rect,
        size: (w as usize, h as usize),
        canvas: (width, height),
    };
    match image {
        DynamicImage::ImageRgb8(i) => render(i.as_raw(), 3, f, a),
        DynamicImage::ImageRgba8(i) => render(i.as_raw(), 4, f, a),
        DynamicImage::ImageRgb16(i) => render(i.as_raw(), 3, f, a),
        DynamicImage::ImageRgba16(i) => render(i.as_raw(), 4, f, a),
        other if other.color().has_alpha() => render(other.to_rgba8().as_raw(), 4, f, a),
        other => render(other.to_rgb8().as_raw(), 3, f, a),
    }
}

#[derive(Clone, Copy)]
struct Frame {
    src: (usize, usize),
    /// `[left, top, width, height]` in source pixels, fractional.
    rect: [f64; 4],
    /// The resized image, centred on the white `canvas`.
    size: (usize, usize),
    canvas: (u32, u32),
}

/// For each output sample: its first input index and normalised Lanczos3 weights. The `out`
/// samples cover the input span `start..start + len` (pixel i spans i..i+1), taps clamped to
/// `0..len_in`.
fn lanczos_weights(len_in: usize, start: f64, len: f64, out: usize) -> Vec<(usize, Vec<f32>)> {
    fn lanczos3(x: f64) -> f64 {
        if x.abs() < 1e-9 {
            1.0
        } else if x.abs() < 3.0 {
            let p = std::f64::consts::PI * x;
            3.0 * p.sin() * (p / 3.0).sin() / (p * p)
        } else {
            0.0
        }
    }
    let scale = len / out as f64;
    let s = scale.max(1.0);
    (0..out)
        .map(|i| {
            let c = start + (i as f64 + 0.5) * scale;
            let lo = ((c - 3.0 * s).floor().max(0.0) as usize).min(len_in - 1);
            let hi = ((c + 3.0 * s).ceil().max(0.0) as usize).clamp(lo + 1, len_in);
            let w: Vec<f64> = (lo..hi)
                .map(|j| lanczos3((j as f64 + 0.5 - c) / s))
                .collect();
            let sum: f64 = w.iter().sum();
            (lo, w.iter().map(|v| (v / sum) as f32).collect())
        })
        .collect()
}

/// Box pre-shrink target: the boxed image is at most about this many times the output size.
/// Measured on a 24 MP photo: 2 lost SSIM on fine texture, and no box at all doubled the time and
/// memory for under 0.0005 SSIM.
const BOX_TARGET: f64 = 3.0;

/// [`compose`] for one pixel layout: `ch` is 3 (RGB) or 4 (RGBA).
///
/// Memory: no float copy of the source. A box filter averages k x k blocks in linear light
/// (premultiplied by alpha) while decoding, with k chosen so the boxed image is at most about
/// [`BOX_TARGET`] times the output. Lanczos3 then resamples it to the output size, across first,
/// one boxed row at a time, so the only large buffer is the across-resampled rows.
fn render<S: Sample>(raw: &[S], ch: usize, f: Frame, a: Adjustments) -> RgbImage {
    use crate::lightcraft as lc;
    let decode: Vec<f32> = (0..=S::MAX)
        .map(|v| lc::srgb_to_linear(v as f32 / S::MAX as f32))
        .collect();
    let [l, t, cw, chh] = f.rect;
    let ((sw, sh), (w, h)) = (f.src, f.size);
    // k never exceeds the source size: k <= ceil(scale / 3) and the crop fits in the source.
    let k = ((cw / w as f64).min(chh / h as f64) / BOX_TARGET)
        .ceil()
        .max(1.0) as usize;
    // The whole k-blocks covering the source span that the Lanczos taps reach.
    let reach = |start: f64, len: f64, out: usize, max: usize| {
        let margin = 3.0 * (len / out as f64).max(k as f64) + k as f64;
        let lo = (start - margin).floor().max(0.0) as usize;
        let hi = ((start + len + margin).ceil() as usize).min(max);
        (lo, ((hi - lo) / k).max(1))
    };
    let (x0, bw) = reach(l, cw, w, sw);
    let (y0, bh) = reach(t, chh, h, sh);
    let xs = lanczos_weights(bw, (l - x0 as f64) / k as f64, cw / k as f64, w);
    let ys = lanczos_weights(bh, (t - y0 as f64) / k as f64, chh / k as f64, h);
    let first = ys[0].0;
    let last = ys
        .iter()
        .map(|(lo, wt)| lo + wt.len())
        .max()
        .unwrap_or(first);
    // Each boxed row the vertical taps use, built from its k source rows, then resampled across.
    let mut wide = vec![0.0f32; (last - first) * w * ch];
    par_rows(&mut wide, w * ch, |r, out| {
        let mut row = vec![0.0f32; bw * ch];
        let by = first + r;
        for sy in y0 + by * k..y0 + (by + 1) * k {
            let src = &raw[(sy * sw + x0) * ch..(sy * sw + x0 + bw * k) * ch];
            for (i, p) in src.chunks_exact(ch).enumerate() {
                let o = &mut row[i / k * ch..][..ch];
                let alpha = if ch == 4 {
                    p[3].index() as f32 / S::MAX as f32
                } else {
                    1.0
                };
                for c in 0..3 {
                    o[c] += decode[p[c].index()] * alpha;
                }
                if ch == 4 {
                    o[3] += alpha;
                }
            }
        }
        let n = 1.0 / (k * k) as f32;
        for (x, (lo, wt)) in xs.iter().enumerate() {
            let o = &mut out[x * ch..][..ch];
            for (j, wv) in wt.iter().enumerate() {
                let p = &row[(lo + j) * ch..][..ch];
                for c in 0..ch {
                    o[c] += wv * n * p[c];
                }
            }
        }
    });
    // Vertical pass, adjustments and alpha over white, straight into the composition.
    let tone = tone(a);
    let encode = lc::table(lc::linear_to_srgb);
    let (cw8, ch8) = f.canvas;
    let (left, top) = ((cw8 as usize - w) / 2, (ch8 as usize - h) / 2);
    let mut rgb = RgbImage::from_pixel(cw8, ch8, Rgb([255; 3]));
    par_rows(&mut rgb, cw8 as usize * 3, |y, out| {
        let Some((lo, wt)) = y.checked_sub(top).and_then(|y| ys.get(y)) else {
            return;
        };
        let mut acc = vec![0.0f32; w * ch];
        for (j, wv) in wt.iter().enumerate() {
            let row = &wide[(lo + j - first) * w * ch..][..w * ch];
            acc.iter_mut().zip(row).for_each(|(a, v)| *a += wv * v);
        }
        for (x, p) in acc.chunks_exact(ch).enumerate() {
            let alpha = if ch == 4 { p[3].clamp(0.0, 1.0) } else { 1.0 };
            let mut c = [0, 1, 2].map(|i| (p[i] / alpha.max(1e-6)).clamp(0.0, 1.0));
            if let Some(tone) = &tone {
                c = tone(c);
            }
            let o = &mut out[(left + x) * 3..][..3];
            for i in 0..3 {
                let v = lc::lut(&encode, c[i]) * alpha + (1.0 - alpha);
                o[i] = (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    });
    rgb
}

fn rotate_cubic(input: &RgbImage, degrees: f64) -> RgbImage {
    let (w, h) = input.dimensions();
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let (s, c) = degrees.to_radians().sin_cos();
    fn weight(v: f64) -> f64 {
        let x = v.abs();
        let a = -0.75;
        if x <= 1.0 {
            (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0
        } else if x < 2.0 {
            a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a
        } else {
            0.0
        }
    }
    RgbImage::from_fn(w, h, |x, y| {
        let dx = x as f64 - cx;
        let dy = y as f64 - cy;
        let sx = c * dx - s * dy + cx;
        let sy = s * dx + c * dy + cy;
        let ix = sx.floor() as i64;
        let iy = sy.floor() as i64;
        let mut values = [0.0; 3];
        for yy in iy - 1..=iy + 2 {
            for xx in ix - 1..=ix + 2 {
                if xx >= 0 && yy >= 0 && xx < w as i64 && yy < h as i64 {
                    let p = input.get_pixel(xx as u32, yy as u32);
                    let factor = weight(sx - xx as f64) * weight(sy - yy as f64);
                    for channel in 0..3 {
                        values[channel] += p[channel] as f64 * factor;
                    }
                }
            }
        }
        Rgb(values.map(|v| v.round().clamp(0.0, 255.0) as u8))
    })
}

pub fn validate_jpeg(bytes: &[u8], width: u32, height: u32, max_bytes: usize) -> Result<()> {
    if bytes.len() > max_bytes {
        bail!("JPEG exceeds printer limit: {} > {max_bytes}", bytes.len());
    }
    if image::guess_format(bytes)? != image::ImageFormat::Jpeg {
        bail!("prepared input must be JPEG");
    }
    let image = ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Jpeg).decode()?;
    if image.dimensions() != (width, height) {
        bail!(
            "JPEG dimensions {:?}, but the printer expects {width}x{height}",
            image.dimensions()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_composite_and_contain() {
        let input = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            10,
            20,
            image::Rgba([255, 0, 0, 0]),
        ));
        let p = prepare(input, 60, 80, 50000, Fit::Contain).unwrap();
        let decoded = image::load_from_memory(&p.jpeg).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (60, 80));
        assert!(decoded.pixels().all(|p| p.0.iter().all(|v| *v >= 250)));
    }
    #[test]
    fn preserves_size_limit_and_checks_input() {
        let p = prepare(DynamicImage::new_rgb8(30, 20), 60, 80, 4000, Fit::Crop).unwrap();
        assert!(p.jpeg.len() <= 4000);
        validate_jpeg(&p.jpeg, 60, 80, 4000).unwrap();
        assert!(validate_jpeg(&p.jpeg, 80, 60, 4000).is_err());
        assert!(prepare(DynamicImage::new_rgb8(3, 2), 60, 80, 1, Fit::Crop).is_err());
    }
    #[test]
    fn adjustments_identity_and_effect() {
        let input = DynamicImage::ImageRgb8(RgbImage::from_fn(8, 4, |x, y| {
            Rgb([x as u8 * 30, y as u8 * 60, 128])
        }));
        let same = adjust(input.clone(), Adjustments::default());
        assert_eq!(same.as_bytes(), input.as_bytes());
        assert_eq!(same.color(), input.color());
        let warmer = adjust(
            input.clone(),
            Adjustments {
                temperature: 50,
                ..Default::default()
            },
        );
        assert_ne!(warmer.to_rgb8().into_raw(), input.to_rgb8().into_raw());
    }
    fn gradient(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| {
            let v = ((x * 255) / w.max(1)) as u8;
            Rgb([
                v,
                ((y * 255) / h.max(1)) as u8,
                if (x / 3 + y / 3) % 2 == 0 { 200 } else { 40 },
            ])
        }))
    }
    #[test]
    fn each_slider_changes_pixels() {
        let d = Adjustments::default;
        for (name, a) in [
            (
                "brightness",
                Adjustments {
                    brightness: 30,
                    ..d()
                },
            ),
            (
                "contrast",
                Adjustments {
                    contrast: 30,
                    ..d()
                },
            ),
            (
                "saturation",
                Adjustments {
                    saturation: 50,
                    ..d()
                },
            ),
            (
                "temperature",
                Adjustments {
                    temperature: 50,
                    ..d()
                },
            ),
        ] {
            let input = gradient(48, 32);
            let out = adjust(input.clone(), a).to_rgb8().into_raw();
            assert_ne!(
                out,
                input.to_rgb8().into_raw(),
                "{name} left the image unchanged"
            );
        }
    }
    #[test]
    fn tone_sliders_keep_black_and_white() {
        let input =
            DynamicImage::ImageRgb8(RgbImage::from_fn(2, 1, |x, _| Rgb([x as u8 * 255; 3])));
        for a in [
            Adjustments {
                brightness: 100,
                ..Default::default()
            },
            Adjustments {
                brightness: -100,
                ..Default::default()
            },
            Adjustments {
                contrast: 100,
                ..Default::default()
            },
            Adjustments {
                contrast: -100,
                ..Default::default()
            },
        ] {
            assert_eq!(
                adjust(input.clone(), a).to_rgb8().into_raw(),
                [0, 0, 0, 255, 255, 255]
            );
        }
    }
    #[test]
    fn full_desaturation_is_grey() {
        let a = Adjustments {
            saturation: -100,
            ..Default::default()
        };
        let out = adjust(gradient(16, 8), a).to_rgb8();
        assert!(out
            .pixels()
            .all(|p| p.0.iter().max().unwrap() - p.0.iter().min().unwrap() <= 1));
    }
    /// `cargo test --release -p outstax-client --lib -- --ignored --nocapture adjust_timing`
    #[test]
    #[ignore]
    fn adjust_timing() {
        let input = gradient(1260, 840);
        let all = Adjustments {
            brightness: 20,
            contrast: 20,
            saturation: 20,
            temperature: 20,
            ..Default::default()
        };
        adjust(input.clone(), all);
        let runs = 5;
        let start = std::time::Instant::now();
        for _ in 0..runs {
            std::hint::black_box(adjust(input.clone(), all));
        }
        println!(
            "adjust 1260x840, all sliders: {:?} per pass",
            start.elapsed() / runs
        );
    }
    /// `pixels` as a PNG (8 or 16 bit, RGB or RGBA) with an optional ICC profile, loaded back.
    fn load_png(
        name: &str,
        pixels: &[u8],
        color: image::ExtendedColorType,
        icc: Option<Vec<u8>>,
    ) -> DynamicImage {
        use image::ImageEncoder;
        let mut png = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut png);
        if let Some(icc) = icc {
            encoder.set_icc_profile(icc).unwrap();
        }
        let n = pixels.len() as u32 / (color.bits_per_pixel() as u32 / 8);
        encoder.write_image(pixels, n, 1, color).unwrap();
        let path = std::env::temp_dir().join(format!("outstax-{}-{name}.png", std::process::id()));
        std::fs::write(&path, png).unwrap();
        let image = load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        image
    }
    #[test]
    fn embedded_profiles_convert_to_srgb() {
        use image::ExtendedColorType as C;
        use moxcms::ColorProfile;
        let p3 = ColorProfile::new_display_p3().encode().unwrap();
        let srgb = ColorProfile::new_srgb().encode().unwrap();
        // Untagged and sRGB-tagged pixels stay byte-identical.
        let pixels: Vec<u8> = (0..=255).flat_map(|v| [v, 255 - v, v / 2]).collect();
        for icc in [None, Some(srgb)] {
            let tagged = icc.is_some();
            let image = load_png("same", &pixels, C::Rgb8, icc);
            assert_eq!(image.as_bytes(), pixels, "sRGB tag {tagged}");
        }
        // The expected sRGB value of a Display P3 colour, by the textbook D65 matrix.
        let lin = |v: f64| crate::lightcraft::srgb_to_linear(v as f32) as f64;
        let enc = |v: f64| crate::lightcraft::linear_to_srgb(v as f32) as f64;
        let p3_to_srgb = [
            [1.224_940_2, -0.224_940_2, 0.0],
            [-0.042_056_9, 1.042_057_1, 0.0],
            [-0.019_637_6, -0.078_636_1, 1.098_273_5],
        ];
        let colour = [180.0, 120.0, 90.0].map(|v: f64| lin(v / 255.0));
        let want: [f64; 3] = std::array::from_fn(|i| {
            enc((0..3).map(|j| p3_to_srgb[i][j] * colour[j]).sum()) * 255.0
        });
        let near = |got: &[f64], what: &str| {
            assert!(
                (0..3).all(|i| (got[i] - want[i]).abs() <= 1.0),
                "{what}: {got:?} vs {want:?}"
            );
        };
        let rgb8 = load_png("p3-8", &[180, 120, 90], C::Rgb8, Some(p3.clone()));
        let wide = |b: &[u8]| -> Vec<f64> { b.iter().map(|&v| v as f64).collect() };
        near(&wide(&rgb8.as_bytes()[..3]), "rgb8");
        let rgba8 = load_png("p3-a", &[180, 120, 90, 77], C::Rgba8, Some(p3.clone()));
        assert_eq!(rgba8.as_bytes()[3], 77);
        near(&wide(&rgba8.as_bytes()[..3]), "rgba8");
        let deep: Vec<u8> = [180u16, 120, 90]
            .iter()
            .flat_map(|v| (v * 257).to_ne_bytes())
            .collect();
        let DynamicImage::ImageRgb16(rgb16) = load_png("p3-16", &deep, C::Rgb16, Some(p3)) else {
            panic!("not 16 bit")
        };
        let got: Vec<f64> = rgb16.as_raw().iter().map(|&v| v as f64 / 257.0).collect();
        near(&got, "rgb16");
    }
    #[test]
    fn quality_search_takes_the_largest_that_fits() {
        let rgb = gradient(120, 90).to_rgb8();
        let size = |q| {
            let mut jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpeg, q)
                .encode_image(&rgb)
                .unwrap();
            jpeg.len()
        };
        for budget in [size(30), size(77), size(77) - 1, 1 << 20] {
            let (jpeg, q) = encode_within(&rgb, budget).unwrap();
            assert!(jpeg.len() <= budget);
            assert_eq!(jpeg.len(), size(q));
            assert!((q + 1..=100).all(|h| size(h) > budget), "{budget}: q{q}");
        }
        assert!(encode_within(&rgb, 10).is_err());
    }
    #[test]
    fn design_preserves_physical_aspect_with_anisotropic_wire_dimensions() {
        let p = prepare_job(
            DynamicImage::new_rgb8(30, 40),
            60,
            80,
            50000,
            Fit::Crop,
            None,
            true,
        )
        .unwrap();
        assert_eq!((p.width, p.height), (60, 160));
        validate_jpeg(&p.jpeg, 60, 160, 50000).unwrap();
    }
    #[test]
    fn fill_samples_the_exact_crop_rectangle() {
        // Linear-light ramps (red across, green down) survive linear-light resampling, so each
        // output pixel must show the ramp at its centre mapped into the crop rectangle.
        let ramp = |v: f64| 0.05 + 0.9 * v;
        let enc = |v: f64| crate::lightcraft::linear_to_srgb(v as f32) as f64 * 255.0;
        for (w, h) in [(900, 500), (500, 900), (601, 799), (45, 25)] {
            let input = DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| {
                let r = enc(ramp((x as f64 + 0.5) / w as f64));
                let g = enc(ramp((y as f64 + 0.5) / h as f64));
                Rgb([r.round() as u8, g.round() as u8, 128])
            }));
            for crop in [
                Crop::default(),
                Crop {
                    x: 0.9,
                    y: 0.1,
                    ..Default::default()
                },
                Crop {
                    x: 0.3,
                    y: 0.7,
                    zoom: 2.0,
                },
            ] {
                let a = Adjustments {
                    crop,
                    ..Default::default()
                };
                let out = compose(input.clone(), 60, 80, Fit::Crop, a);
                let (_, [l, t, cw, ch]) = crop.fit(w, h, 0.75);
                // Away from the image edges, where the Lanczos taps are cut short.
                let reach = 3.0 * (cw / 60.0).max(1.0) + 1.0;
                let inside = |v: f64, max: u32| v > reach && v < max as f64 - reach;
                for (x, y, p) in out.enumerate_pixels() {
                    let sx = l + (x as f64 + 0.5) * cw / 60.0;
                    let sy = t + (y as f64 + 0.5) * ch / 80.0;
                    let ctx = format!("{w}x{h} {crop:?} at {x},{y}: {p:?}");
                    if inside(sx, w) {
                        let want = enc(ramp(sx / w as f64));
                        assert!((p[0] as f64 - want).abs() <= 1.5, "{ctx} vs {want:.1}");
                    }
                    if inside(sy, h) {
                        let want = enc(ramp(sy / h as f64));
                        assert!((p[1] as f64 - want).abs() <= 1.5, "{ctx} vs {want:.1}");
                    }
                }
            }
        }
    }
    #[test]
    fn preview_and_print_share_one_deterministic_pipeline() {
        let input = gradient(900, 500);
        // The print wrappers and the preview's call agree byte for byte, run to run.
        let print = prepare_job(input.clone(), 60, 80, 50000, Fit::Crop, None, false).unwrap();
        for _ in 0..2 {
            let preview = prepare_with_options(
                input.clone(),
                60,
                80,
                50000,
                Fit::Crop,
                Processing::default(),
            )
            .unwrap();
            assert_eq!(preview.jpeg, print.jpeg);
        }
        assert!(tone(Adjustments::default()).is_none());
        // On smooth content, adjusting after the resize looks like adjusting before it.
        let input = DynamicImage::ImageRgb8(RgbImage::from_fn(900, 500, |x, y| {
            Rgb([(x * 255 / 900) as u8, (y * 255 / 500) as u8, 128])
        }));
        let a = Adjustments {
            brightness: 20,
            contrast: 30,
            saturation: -40,
            temperature: 50,
            quarter_turns: 1,
            ..Default::default()
        };
        let decode = |p: PreparedImage| image::load_from_memory(&p.jpeg).unwrap().to_rgb8();
        let before = decode(prepare(adjust(input.clone(), a), 60, 80, 1 << 20, Fit::Crop).unwrap());
        let processing = Processing {
            adjust: a,
            ..Default::default()
        };
        let after =
            decode(prepare_with_options(input, 60, 80, 1 << 20, Fit::Crop, processing).unwrap());
        let n = before.as_raw().len() as f64;
        let diff: f64 = before
            .as_raw()
            .iter()
            .zip(after.as_raw())
            .map(|(a, b)| a.abs_diff(*b) as f64)
            .sum();
        assert!(diff / n < 1.0, "mean difference {}", diff / n);
    }
    #[test]
    fn crop_keeps_aspect_and_stays_inside() {
        for (w, h) in [(4000, 3000), (3000, 4000), (801, 799), (7, 5)] {
            for aspect in [0.75, 0.375, 1.0, 1.6] {
                for zoom in [0.0, 1.0, 2.7, 4.0, 100.0] {
                    for (x, y) in [(-5.0, -5.0), (0.0, 1.0), (0.5, 0.5), (1.0, 0.0), (7.0, 7.0)] {
                        let (crop, [l, t, cw, ch]) = Crop { x, y, zoom }.fit(w, h, aspect);
                        let ctx = format!("{w}x{h} aspect {aspect} {x},{y} zoom {zoom}");
                        assert!((cw / ch - aspect).abs() < 1e-9, "{ctx}");
                        assert!(l >= 0.0 && t >= 0.0, "{ctx}");
                        assert!(
                            l + cw <= w as f64 + 1e-9 && t + ch <= h as f64 + 1e-9,
                            "{ctx}"
                        );
                        assert!((1.0..=Crop::MAX_ZOOM).contains(&crop.zoom), "{ctx}");
                        // At 1x the crop spans the image on one axis.
                        if zoom <= 1.0 {
                            assert!(cw == w as f64 || ch == h as f64, "{ctx}");
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn crop_selects_the_same_region_with_design_print() {
        // Bands: red x < 120, green x < 240, blue beyond. The film's 3:4 crop pinned to the left
        // covers exactly red then green. A wrong aspect (Design Print's 600x1600 wire size, or a
        // too-wide one) would show only red, or blue too.
        let input = DynamicImage::ImageRgb8(RgbImage::from_fn(480, 320, |x, _| {
            Rgb(match x {
                0..120 => [255, 0, 0],
                120..240 => [0, 255, 0],
                _ => [0, 0, 255],
            })
        }));
        let crop = Crop {
            x: 0.0,
            ..Default::default()
        };
        for design in [false, true] {
            let processing = Processing {
                design,
                adjust: Adjustments {
                    crop,
                    ..Default::default()
                },
                ..Default::default()
            };
            let p =
                prepare_with_options(input.clone(), 60, 80, 50000, Fit::Crop, processing).unwrap();
            assert_eq!((p.width, p.height), (60, if design { 160 } else { 80 }));
            let out = image::load_from_memory(&p.jpeg).unwrap().to_rgb8();
            // Keep away from the band edge and from the edges the Design Print warp rotates in.
            let (w, h) = out.dimensions();
            for y in h / 8..h * 7 / 8 {
                for (xs, channel) in [(w / 8..w * 3 / 8, 0), (w * 5 / 8..w * 7 / 8, 1)] {
                    for x in xs {
                        let p = out.get_pixel(x, y).0;
                        assert!(
                            (0..3).all(|i| if i == channel { p[i] > 200 } else { p[i] < 60 }),
                            "design {design} {x},{y}: {p:?}"
                        );
                    }
                }
            }
        }
    }
}
