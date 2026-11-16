//! Independent port of the optional Link+ native USM algorithm. Floating-point
//! convolution/encoding may differ from OpenCV; this is separate from FDIA auto enhancement.
use anyhow::{bail, Result};
use image::{Rgb, RgbImage};
#[derive(Debug, Clone, Copy)]
pub struct Unsharp {
    pub sigma: f64,
    pub amount: f64,
    pub threshold: f64,
}
impl Default for Unsharp {
    fn default() -> Self {
        Self {
            sigma: 3.0,
            amount: 0.5,
            threshold: 0.8,
        }
    }
}
fn linear(x: f64) -> f64 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn nonlinear(x: f64) -> f64 {
    if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}
fn reflect(mut x: isize, n: usize) -> usize {
    if n == 1 {
        return 0;
    }
    let n = n as isize;
    while x < 0 || x >= n {
        if x < 0 {
            x = -x;
        } else {
            x = 2 * n - 2 - x;
        }
    }
    x as usize
}
fn gaussian(input: &[Vec<f32>], width: usize, height: usize, sigma: f64) -> Vec<Vec<f32>> {
    // OpenCV's auto-sized kernel for CV_32F: round(sigma*8+1), made odd.
    let size = ((sigma * 8.0 + 1.0).round_ties_even() as usize) | 1;
    let radius = size / 2;
    let mut weights: Vec<f64> = (0..size)
        .map(|i| (-((i as f64 - radius as f64).powi(2)) / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f64 = weights.iter().sum();
    for v in &mut weights {
        *v /= total;
    }
    let weights: Vec<f32> = weights.into_iter().map(|x| x as f32).collect();
    let channels = input.len();
    let mut temp = vec![vec![0.0; width * height]; channels];
    let mut result = temp.clone();
    for c in 0..channels {
        for y in 0..height {
            for x in 0..width {
                let mut value = 0.0f32;
                for (k, weight) in weights.iter().enumerate() {
                    value += input[c]
                        [y * width + reflect(x as isize + k as isize - radius as isize, width)]
                        * weight;
                }
                temp[c][y * width + x] = value;
            }
        }
        for y in 0..height {
            for x in 0..width {
                let mut value = 0.0f32;
                for (k, weight) in weights.iter().enumerate() {
                    value += temp[c]
                        [reflect(y as isize + k as isize - radius as isize, height) * width + x]
                        * weight;
                }
                result[c][y * width + x] = value;
            }
        }
    }
    result
}
pub fn apply(input: &RgbImage, settings: Unsharp) -> Result<RgbImage> {
    if !settings.sigma.is_finite()
        || settings.sigma <= 0.0
        || settings.sigma > 32.0
        || !settings.amount.is_finite()
        || !(0.0..=10.0).contains(&settings.amount)
        || !settings.threshold.is_finite()
        || !(0.0..=1.0).contains(&settings.threshold)
        || input.width() == 0
        || input.height() == 0
    {
        bail!("invalid unsharp settings/image");
    }
    let (width, height) = (input.width() as usize, input.height() as usize);
    let mut source = (0..3)
        .map(|_| Vec::with_capacity(width * height))
        .collect::<Vec<_>>();
    for pixel in input.pixels() {
        for c in 0..3 {
            source[c].push(linear(f64::from(pixel[c]) / 255.0) as f32);
        }
    }
    let blurred = gaussian(&source, width, height, settings.sigma);
    let difference: Vec<Vec<f32>> = (0..3)
        .map(|c| {
            source[c]
                .iter()
                .zip(&blurred[c])
                .map(|(a, b)| a - b)
                .collect()
        })
        .collect();
    let mask = if settings.threshold > 0.0 {
        let mask: Vec<f32> = (0..width * height)
            .map(|i| {
                let luminance = 2.0
                    * (0.222488 * difference[0][i].abs()
                        + 0.716904 * difference[1][i].abs()
                        + 0.060608 * difference[2][i].abs());
                if nonlinear(f64::from(luminance)) > settings.threshold {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        gaussian(&[mask], width, height, 1.0).remove(0)
    } else {
        vec![1.0; width * height]
    };
    Ok(RgbImage::from_fn(width as u32, height as u32, |x, y| {
        let i = y as usize * width + x as usize;
        Rgb(std::array::from_fn(|c| {
            let detail = difference[c][i] * mask[i];
            let value = (f64::from(source[c][i]) + settings.amount * f64::from(detail)) as f32;
            let nonlinear = nonlinear(f64::from(value)) as f32;
            (f64::from(nonlinear) * 255.0)
                .round_ties_even()
                .clamp(0.0, 255.0) as u8
        }))
    }))
}
