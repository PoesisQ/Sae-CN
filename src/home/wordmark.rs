//! Print the current cover frame through the letters with the cover's negative.
//! The outline is sampled at physical display resolution, independently of the grain.
use crate::CoverRect;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::sync::OnceLock;

const OUTLINE: &[u8] = include_bytes!("../../assets/home/wordmark.spans");
const BANDS: usize = 1024;
const SAMPLES: usize = 4;

struct SilverInk {
    linear: [u16; 256],
    negative: [u8; 4096],
}

impl SilverInk {
    fn get() -> &'static Self {
        static TABLES: OnceLock<SilverInk> = OnceLock::new();
        TABLES.get_or_init(|| Self {
            linear: std::array::from_fn(|i| {
                let v = i as f64 / 255.;
                let light = if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                };
                (light * 4095.).round() as u16
            }),
            negative: std::array::from_fn(|i| {
                let light = 1. - i as f64 / 4095.;
                let v = if light <= 0.0031308 {
                    12.92 * light
                } else {
                    1.055 * light.powf(1. / 2.4) - 0.055
                };
                (v * 255.).round() as u8
            }),
        })
    }

    fn gray(&self, r: u8, g: u8, b: u8) -> u8 {
        // Rec. 709 luminance in linear light, then back to a neutral sRGB ink.
        let linear = |i: u8| u32::from(self.linear[usize::from(i)]);
        let y = (13933 * linear(r) + 46871 * linear(g) + 4732 * linear(b) + 32768) >> 16;
        self.negative[y as usize]
    }
}

fn word(i: usize) -> usize {
    u16::from_le_bytes([OUTLINE[i * 2], OUTLINE[i * 2 + 1]]) as usize
}

fn coverage(row: usize, height: usize, values: &mut [f32]) {
    values.fill(0.);
    let width = values.len() as f32;
    for sample in 0..SAMPLES {
        let band = (2 * (row * SAMPLES + sample) + 1) * BANDS / (2 * height * SAMPLES);
        for edge in (word(band)..word(band + 1)).step_by(2) {
            let a = word(BANDS + 1 + edge) as f32 * width / 65535.;
            let b = word(BANDS + 2 + edge) as f32 * width / 65535.;
            for (offset, v) in values[a as usize..(b.ceil() as usize)]
                .iter_mut()
                .enumerate()
            {
                let left = (a as usize + offset) as f32;
                *v += (b.min(left + 1.) - a.max(left)).max(0.);
            }
        }
    }
}

struct Run {
    row: usize,
    start: usize,
    end: usize,
}

pub(super) struct Stamp {
    size: (usize, usize),
    runs: Vec<Run>,
    geometry: [usize; 9],
    xs: Vec<usize>,
    ys: Vec<usize>,
    buffers: [SharedPixelBuffer<Rgba8Pixel>; 2],
    next: usize,
}

impl Stamp {
    fn new(w: usize, h: usize) -> Self {
        let mut base = SharedPixelBuffer::<Rgba8Pixel>::new(w as u32, h as u32);
        let mut runs = Vec::new();
        let mut aa = vec![0.; w];
        for (y, row) in base.make_mut_slice().chunks_exact_mut(w).enumerate() {
            coverage(y, h, &mut aa);
            let mut begin = 0;
            for (x, (p, v)) in row.iter_mut().zip(&aa).enumerate() {
                p.a = (v * (255. / SAMPLES as f32)).round() as u8;
                if p.a == 0 {
                    if begin < x {
                        runs.push(Run {
                            row: y,
                            start: begin,
                            end: x,
                        });
                    }
                    begin = x + 1;
                }
            }
            if begin < w {
                runs.push(Run {
                    row: y,
                    start: begin,
                    end: w,
                });
            }
        }
        Self {
            size: (w, h),
            runs,
            geometry: [0; 9],
            xs: Vec::with_capacity(w),
            ys: Vec::with_capacity(h),
            buffers: [base.clone(), base],
            next: 0,
        }
    }

    fn paint(
        &mut self,
        bytes: &[u8],
        stride: usize,
        source: (usize, usize),
        rect: &CoverRect,
        silver: Option<&SilverInk>,
    ) -> Image {
        let (w, h) = self.size;
        let buffer = &mut self.buffers[self.next];
        let output = buffer.make_mut_slice();
        // Same 8.8 texture coordinates used by Slint's Windows software renderer
        // for the full-bleed Image. A crop resized independently drifts by a grain.
        let indices = |offset: f32, extent: f32, length: usize, viewport: f32, texels: usize| {
            let increment = ((texels * 256) / (viewport.round().max(1.) as usize)).max(1);
            let start = offset.round().max(0.) as usize;
            let span = extent.round().max(1.) as usize;
            (0..length).map(move |i| (((start + i * span / length) * increment) >> 8) % texels)
        };
        let geometry = [
            rect.x.round() as usize,
            rect.y.round() as usize,
            rect.w.round() as usize,
            rect.h.round() as usize,
            rect.window_w.round() as usize,
            rect.window_h.round() as usize,
            source.0,
            source.1,
            stride,
        ];
        if geometry != self.geometry {
            self.xs.clear();
            self.ys.clear();
            self.xs
                .extend(indices(rect.x, rect.w, w, rect.window_w, source.0).map(|x| x * stride));
            self.ys.extend(
                indices(rect.y, rect.h, h, rect.window_h, source.1).map(|y| y * source.0 * stride),
            );
            self.geometry = geometry;
        }
        for run in &self.runs {
            let src = &bytes[self.ys[run.row]..];
            let line = &mut output[run.row * w + run.start..run.row * w + run.end];
            for (px, &x) in line.iter_mut().zip(&self.xs[run.start..run.end]) {
                let (r, g, b) = (src[x], src[x + 1], src[x + 2]);
                let ink = if let Some(table) = silver {
                    [table.gray(r, g, b); 3]
                } else {
                    let shift = 255 - i16::from(r.max(g).max(b)) - i16::from(r.min(g).min(b));
                    [r, g, b].map(|c| (i16::from(c) + shift) as u8)
                };
                let alpha = u16::from(px.a);
                [px.r, px.g, px.b] = if alpha == 255 {
                    ink
                } else {
                    ink.map(|c| ((u16::from(c) * alpha + 127) / 255) as u8)
                };
            }
        }
        self.next ^= 1;
        Image::from_rgba8_premultiplied(buffer.clone())
    }
}

pub(super) fn render(
    cache: &mut Option<Stamp>,
    frame: Image,
    rect: CoverRect,
    monochrome: bool,
) -> Image {
    if rect.w < 1. || rect.h < 1. || rect.window_w < 1. || rect.window_h < 1. {
        return Image::default();
    }
    // Covers through 250% Windows scaling retain a physical sample on every
    // pixel. The allocation stays bounded on very large custom displays.
    let scale = (896. / rect.w).min(1.);
    let size = (
        (rect.w * scale).round().max(1.) as usize,
        (rect.h * scale).round().clamp(1., 484.) as usize,
    );
    if cache.as_ref().is_none_or(|stamp| stamp.size != size) {
        *cache = Some(Stamp::new(size.0, size.1));
    }
    let stamp = cache.as_mut().unwrap();
    let silver = monochrome.then(SilverInk::get);
    if let Some(rgb) = frame.to_rgb8().filter(|p| p.width() > 0 && p.height() > 0) {
        return stamp.paint(
            rgb.as_bytes(),
            3,
            (rgb.width() as usize, rgb.height() as usize),
            &rect,
            silver,
        );
    }
    frame
        .to_rgba8()
        .filter(|p| p.width() > 0 && p.height() > 0)
        .map_or_else(Image::default, |rgba| {
            stamp.paint(
                rgba.as_bytes(),
                4,
                (rgba.width() as usize, rgba.height() as usize),
                &rect,
                silver,
            )
        })
}
