use anyhow::{bail, Context, Result};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, ImageDecoder,
    ImageReader, Rgb, RgbImage,
};
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

/// Decode with allocation limits, apply EXIF orientation, composite alpha on white.
pub fn load(path: &Path) -> Result<DynamicImage> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut decoder = reader
        .into_decoder()
        .context("unsupported image format. Use JPEG, PNG or WebP")?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// Slider adjustments of our own (not vendor processing), applied to the decoded image before
/// composition. Each value is -100..=100; zero is neutral, and all-zero returns the image unchanged.
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
    /// Fill-mode framing. [`adjust`] ignores it: pass it to [`Processing::crop`].
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

/// Rotate, then adjust in linear light, in LightCraft's stage order (crates/pipeline/src/lib.rs):
/// 1. decode sRGB to linear;
/// 2. scene linear: Color temperature (Bradford white balance, neutral luminance kept);
/// 3. tone: Brightness (a mid-tone gamma that keeps black and white), then Contrast (an S-curve on
///    luminance anchored at black, grey and white, colour ratios kept);
/// 4. colour: Saturation (OkLab chroma, so lightness and hue stay);
/// 5. display: gamut map (desaturate out-of-range colours towards their luminance), sRGB encode.
///
/// All-zero settings return the input unchanged.
pub fn adjust(image: DynamicImage, a: Adjustments) -> DynamicImage {
    use crate::lightcraft as lc;
    let image = match a.quarter_turns % 4 {
        1 => image.rotate90(),
        2 => image.rotate180(),
        3 => image.rotate270(),
        _ => image,
    };
    let [brightness, contrast, saturation] =
        [a.brightness, a.contrast, a.saturation].map(|v| v as f32 / 100.0);
    if [a.brightness, a.contrast, a.saturation, a.temperature] == [0; 4] {
        return image;
    }
    let decode: [f32; 256] = std::array::from_fn(|i| lc::srgb_to_linear(i as f32 / 255.0));
    let encode = lc::table(lc::linear_to_srgb);
    let gamma = lc::brightness_gamma(brightness);
    let curve = lc::table(|y| lc::contrast_curve(contrast, y));
    let wb = (a.temperature != 0).then(|| lc::white_balance(a.temperature as f64));
    let pixel = |p: &mut [u8; 4]| {
        let mut c = [0, 1, 2].map(|i| decode[p[i] as usize]);
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
        c = lc::gamut_map(c);
        for (i, v) in c.into_iter().enumerate() {
            p[i] = (lc::lut(&encode, v) * 255.0 + 0.5) as u8;
        }
    };
    // Row bands on scoped std threads (no rayon); each pixel is independent, alpha untouched.
    let mut rgba = image.into_rgba8();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    let band = (rgba.height() as usize).div_ceil(threads).max(1) * rgba.width() as usize * 4;
    let pixel = &pixel;
    std::thread::scope(|s| {
        for chunk in rgba.chunks_mut(band.max(4)) {
            s.spawn(move || chunk.as_chunks_mut::<4>().0.iter_mut().for_each(pixel));
        }
    });
    DynamicImage::ImageRgba8(rgba)
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
    pub crop: Crop,
}
/// Composition -> optional linear-light USM -> optional Rich LUT -> Design warp
/// -> JPEG. This is an independent pipeline; FDIA enhancement is separate.
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
        crop,
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
    let rgba = compose(image, width, height, fit, crop).to_rgba8();
    let mut rgb = RgbImage::from_pixel(width, height, Rgb([255, 255, 255]));
    let left = (width - rgba.width()) / 2;
    let top = (height - rgba.height()) / 2;
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        rgb.put_pixel(
            left + x,
            top + y,
            Rgb(std::array::from_fn(|i| {
                ((p[i] as u32 * a + 255 * (255 - a) + 127) / 255) as u8
            })),
        );
    }
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
    // Descending quality avoids relying on a monotonic JPEG-size assumption.
    for quality in (1..=95).rev() {
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality).encode_image(&rgb)?;
        if jpeg.len() <= max_bytes {
            return Ok(PreparedImage {
                jpeg,
                width: output_width,
                height: output_height,
                quality,
            });
        }
    }
    bail!("image cannot fit the printer's byte limit at minimum JPEG quality")
}

/// Scale to the composition size: Fit shrinks to fit inside, Fill covers it through `crop`.
/// The default crop is exactly the centred `resize_to_fill`.
fn compose(image: DynamicImage, width: u32, height: u32, fit: Fit, crop: Crop) -> DynamicImage {
    let filter = FilterType::Lanczos3;
    if let Fit::Contain = fit {
        return image.resize(width, height, filter);
    }
    let (iw, ih) = image.dimensions();
    let (crop, [l, t, w, h]) = crop.fit(iw, ih, width as f64 / height as f64);
    if crop == Crop::default() {
        return image.resize_to_fill(width, height, filter);
    }
    let w = (w.round() as u32).clamp(1, iw);
    let h = (h.round() as u32).clamp(1, ih);
    let l = (l.round() as u32).min(iw - w);
    let t = (t.round() as u32).min(ih - h);
    image
        .crop_imm(l, t, w, h)
        .resize_exact(width, height, filter)
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
    fn default_crop_is_the_centred_fill() {
        for (w, h) in [(90, 50), (50, 90), (60, 80)] {
            let input = gradient(w, h);
            let old = input.resize_to_fill(60, 80, FilterType::Lanczos3);
            // The axis the fill already spans cannot move, so x/y there clamp back to centre.
            for crop in [
                Crop::default(),
                Crop {
                    x: 0.9,
                    ..Default::default()
                },
                Crop {
                    y: 0.1,
                    ..Default::default()
                },
            ]
            .into_iter()
            .filter(|c| c.fit(w, h, 0.75).0 == Crop::default())
            {
                let new = compose(input.clone(), 60, 80, Fit::Crop, crop);
                assert_eq!(new.as_bytes(), old.as_bytes(), "{w}x{h} {crop:?}");
            }
        }
        let input = gradient(90, 50);
        let old = prepare(input.clone(), 60, 80, 50000, Fit::Crop)
            .unwrap()
            .jpeg;
        let processing = Processing {
            crop: Crop::default(),
            ..Default::default()
        };
        let new = prepare_with_options(input, 60, 80, 50000, Fit::Crop, processing).unwrap();
        assert_eq!(new.jpeg, old);
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
                crop,
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
