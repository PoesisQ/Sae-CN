//! The reference effects have a smooth colour field and stationary, screen-space grain.
//! Evaluate them separately so the five Plasma taps can share neighbouring samples.
use miniz_oxide::inflate::decompress_slice_iter_to_slice;
use slint::{Image, Rgb8Pixel, SharedPixelBuffer};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Plasma,
    Blue,
}

type Color = [i16; 3];
const ATLAS: &[u8] = include_bytes!("../../assets/home/blue.field");
const GRAIN: &[u8] = include_bytes!("../../assets/home/blue-grain.zlib");
const BLUE_W: usize = 160;
const BLUE_H: usize = 90;
const FRAME_BYTES: usize = BLUE_W * BLUE_H * 3;
const DIRECTORY: usize = 54 * 4;
const GRAIN_W: usize = 640;
const GRAIN_H: usize = 360;
pub(super) const BLUE_ONE_WAY_SECONDS: f32 = 16.25;

fn inflate(source: &[u8], output: &mut [u8]) -> bool {
    decompress_slice_iter_to_slice(output, std::iter::once(source), true, false) == Ok(output.len())
}

struct Keyframe {
    index: usize,
    data: Vec<u8>,
}
impl Keyframe {
    fn empty() -> Self {
        Self {
            index: usize::MAX,
            data: vec![0; FRAME_BYTES],
        }
    }
    fn read(&mut self, index: usize) -> bool {
        if self.index == index {
            return true;
        }
        let offset = |i: usize| {
            DIRECTORY + u32::from_le_bytes(ATLAS[4 * i..4 * i + 4].try_into().unwrap()) as usize
        };
        let Some(stream) = ATLAS.get(offset(index)..offset(index + 1)) else {
            return false;
        };
        if !inflate(stream, &mut self.data) {
            return false;
        }
        for row in self.data.chunks_exact_mut(BLUE_W * 3) {
            for i in 3..row.len() {
                row[i] = row[i].wrapping_add(row[i - 3]);
            }
        }
        self.index = index;
        true
    }
}
struct Blue {
    first: Keyframe,
    second: Keyframe,
}
impl Blue {
    fn sample(&mut self, t: f32, field: &mut [Color]) -> bool {
        let cycle = 2. * BLUE_ONE_WAY_SECONDS;
        let phase = t.rem_euclid(cycle);
        let frame = phase.min(cycle - phase) * (52. / BLUE_ONE_WAY_SECONDS);
        let a = frame.floor() as usize;
        let b = (a + 1).min(52);
        if self.second.index == a || self.first.index == b {
            std::mem::swap(&mut self.first, &mut self.second);
        }
        if !self.first.read(a) || !self.second.read(b) {
            return false;
        }
        let f = (frame.fract() * 256.).round() as i32;
        for ((dst, lo), hi) in field
            .iter_mut()
            .zip(self.first.data.chunks_exact(3))
            .zip(self.second.data.chunks_exact(3))
        {
            for c in 0..3 {
                let lo = (i32::from(lo[c]) * 4).min(255);
                let hi = (i32::from(hi[c]) * 4).min(255);
                dst[c] = (lo + (((hi - lo) * f + 128) >> 8)) as i16;
            }
        }
        true
    }
}

struct Plasma {
    palette: Vec<Color>,
    samples: Vec<Color>,
    vignette: Vec<u16>,
}
impl Plasma {
    fn new(w: usize, h: usize) -> Self {
        let inks = [
            [0.086, 0.043, 0.043],
            [0.761, 0.251, 0.165],
            [0.957, 0.616, 0.216],
            [1.0, 0.910, 0.761],
        ];
        let (sn, cs) = 3.04f32.sin_cos();
        let palette = (0..1024)
            .map(|i| {
                let f = i as f32 * (3. / 1023.);
                let k = (f as usize).min(2);
                let a = smooth(f - k as f32);
                let [r, g, b] = std::array::from_fn(|c| {
                    (inks[k][c] + (inks[k + 1][c] - inks[k][c]) * a - 0.5) * 0.92 + 0.5
                });
                let y = 0.299 * r + 0.587 * g + 0.114 * b;
                let u = 0.596 * r - 0.274 * g - 0.322 * b;
                let v = 0.211 * r - 0.523 * g + 0.312 * b;
                let (u, v) = (u * cs - v * sn, u * sn + v * cs);
                [
                    y + 0.956 * u + 0.621 * v,
                    y - 0.272 * u - 0.647 * v,
                    y - 1.106 * u + 1.703 * v,
                ]
                .map(|s| ((s - 0.50) * 255.).round() as i16)
            })
            .collect();
        let mut vignette = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 + 0.5) / w as f32 - 0.5;
                let dy = (y as f32 + 0.5) / h as f32 - 0.5;
                let vd = ((dx * dx + dy * dy) * 2.).sqrt();
                vignette.push(
                    ((1. - 0.61 * smooth(((vd - 0.35) / 0.65).clamp(0., 1.))) * 256.).round()
                        as u16,
                );
            }
        }
        Self {
            palette,
            samples: vec![[0; 3]; (w + 4) * (h + 4)],
            vignette,
        }
    }
    fn sample(&mut self, t: f32, w: usize, h: usize, out: &mut [Color]) {
        let t = t * 0.86;
        let step = 1.5 / w.min(h) as f32;
        let ox = 0.16 * (t * 0.31).sin();
        let oy = 0.16 * (t * 0.23).cos();
        let stride = w + 4;
        for (y, row) in self.samples.chunks_exact_mut(stride).enumerate() {
            let py = (h as f32 * 0.5 - y as f32 + 1.5) * step + oy;
            let sy = (py * (4.88 * 0.8) - t * 0.7).sin();
            for (x, dst) in row.iter_mut().enumerate() {
                let px = (x as f32 - w as f32 * 0.5 - 1.5) * step + ox;
                let v = (px * 4.88 + t).sin()
                    + sy
                    + ((px + py) * (4.88 * 0.6) + t * 0.5).sin()
                    + ((px * px + py * py).sqrt() * (4.88 * 1.2) - t).sin();
                let idx = ((1. + (v + 7.).sin()) * 511.5).round() as usize;
                *dst = self.palette[idx.min(1023)];
            }
        }
        // The shader offsets each tap by 0.016 * 1.5; at 125 cells that is two cells.
        for (i, dst) in out.iter_mut().enumerate() {
            let p = (i / w + 2) * stride + i % w + 2;
            for (c, channel) in dst.iter_mut().enumerate() {
                let at = |j: usize| i32::from(self.samples[j][c]);
                let weighted = 9 * at(p)
                    + 4 * (at(p - 2) + at(p + 2) + at(p - 2 * stride) + at(p + 2 * stride));
                *channel = ((weighted * i32::from(self.vignette[i])) / (25 * 256)) as i16;
            }
        }
    }
}
fn smooth(x: f32) -> f32 {
    x * x * (3. - 2. * x)
}

enum Source {
    Plasma(Plasma),
    Blue(Blue),
}
#[derive(Clone, Copy)]
struct Sample {
    start: usize,
    mix: i32,
}

fn axis(length: usize, samples: usize) -> Vec<Sample> {
    (0..length)
        .map(|i| {
            let x = (((i as f32 + 0.5) * samples as f32) / length as f32 - 0.5)
                .clamp(0., (samples - 1) as f32);
            let start = (x as usize).min(samples - 2);
            Sample {
                start,
                mix: ((x - start as f32) * 256.).round() as i32,
            }
        })
        .collect()
}

pub(super) struct Field {
    pub size: (usize, usize),
    grid: (usize, usize),
    source: Source,
    colors: Vec<Color>,
    row: Vec<Color>,
    x: Vec<Sample>,
    y: Vec<Sample>,
    grain: Vec<i8>,
    buffers: [SharedPixelBuffer<Rgb8Pixel>; 2],
    next: usize,
}

impl Field {
    pub fn dimensions(width: u32, height: u32) -> (usize, usize) {
        let scale = (800. / width.max(1) as f32)
            .min(520. / height.max(1) as f32)
            .min(1.);
        (
            (width as f32 * scale).round().max(2.) as usize,
            (height as f32 * scale).round().max(2.) as usize,
        )
    }
    pub fn new(kind: Kind, size: (usize, usize)) -> Option<Self> {
        let (w, h) = size;
        let shortest = w.min(h) as f32;
        let (gw, gh) = match kind {
            Kind::Blue => (BLUE_W, BLUE_H),
            Kind::Plasma => (
                ((w as f32 / shortest * 125.).round() as usize).clamp(2, 768),
                ((h as f32 / shortest * 125.).round() as usize).clamp(2, 768),
            ),
        };
        let mut grain = Vec::with_capacity(w * h);
        let source = match kind {
            Kind::Blue => {
                let mut original = vec![0u8; GRAIN_W * GRAIN_H];
                if !inflate(GRAIN, &mut original) {
                    return None;
                }
                // The atlas grain grows stronger toward its foot. Continue with
                // equally textured rows when the finer pitch runs past that edge.
                let sample = |i, size, count| (2 * i + 1) * count * 3 / (size * 5);
                let columns: Vec<usize> = (0..w).map(|x| sample(x, w, GRAIN_W) % GRAIN_W).collect();
                for y in 0..h {
                    let row = sample(y, h, GRAIN_H);
                    let start = (if row < GRAIN_H { row } else { row - GRAIN_H / 2 }) * GRAIN_W;
                    grain.extend(
                        columns
                            .iter()
                            .map(|&x| (i16::from(original[start + x]) - 128) as i8),
                    );
                }
                Source::Blue(Blue {
                    first: Keyframe::empty(),
                    second: Keyframe::empty(),
                })
            }
            Kind::Plasma => {
                for y in 0..h {
                    for x in 0..w {
                        let a = ((x as f32 + 119.5) * 0.1031).fract();
                        let b = ((h as f32 - y as f32 + 216.5) * 0.1031).fract();
                        let dot = a * (b + 33.33) + b * (a + 33.33) + a * (a + 33.33);
                        let hash = (((a + dot) + (b + dot)) * (a + dot)).fract();
                        grain.push(((hash - 0.5) * (0.35 * 255.)).round() as i8);
                    }
                }
                Source::Plasma(Plasma::new(gw, gh))
            }
        };
        Some(Self {
            size,
            grid: (gw, gh),
            source,
            colors: vec![[0; 3]; gw * gh],
            row: vec![[0; 3]; gw],
            x: axis(w, gw),
            y: axis(h, gh),
            grain,
            buffers: std::array::from_fn(|_| SharedPixelBuffer::new(w as u32, h as u32)),
            next: 0,
        })
    }

    pub fn render(&mut self, time: f32) -> Option<Image> {
        let (gw, gh) = self.grid;
        match &mut self.source {
            Source::Plasma(p) => p.sample(time, gw, gh, &mut self.colors),
            Source::Blue(b) => {
                if !b.sample(time, &mut self.colors) {
                    return None;
                }
            }
        }
        let buffer = &mut self.buffers[self.next];
        let pixels = buffer.make_mut_bytes();
        let w = self.size.0;
        for (y, target) in pixels.chunks_exact_mut(w * 3).enumerate() {
            let iy = self.y[y];
            let top = &self.colors[iy.start * gw..][..gw];
            let bottom = &self.colors[(iy.start + 1) * gw..][..gw];
            for ((row, a), b) in self.row.iter_mut().zip(top).zip(bottom) {
                for c in 0..3 {
                    row[c] = a[c] + ((i32::from(b[c] - a[c]) * iy.mix + 128) >> 8) as i16;
                }
            }
            let noise = &self.grain[y * w..][..w];
            for ((out, ix), n) in target.chunks_exact_mut(3).zip(&self.x).zip(noise) {
                let [a, b] = [self.row[ix.start], self.row[ix.start + 1]];
                for c in 0..3 {
                    let v = i32::from(a[c])
                        + ((i32::from(b[c] - a[c]) * ix.mix + 128) >> 8)
                        + i32::from(*n);
                    out[c] = v.clamp(0, 255) as u8;
                }
            }
        }
        let image = Image::from_rgb8(buffer.clone());
        self.next ^= 1;
        Some(image)
    }
}
