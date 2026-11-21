//! Print the current cover frame through the letters with the cover's negative.
//! The outline is sampled at physical display resolution, independently of the grain.
use crate::CoverRect;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

const OUTLINE: &[u8] = include_bytes!("../../assets/home/wordmark.spans");
const BANDS: usize = 1024;
const SAMPLES: usize = 4;

const PITCH: usize = 16;
const CROSSOVER: u8 = 117;
const DEADBAND: u8 = 6;

fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((54 * u16::from(r) + 183 * u16::from(g) + 19 * u16::from(b) + 128) >> 8) as u8
}

// The broad light field chooses the ink; the original texel supplies its grain
// and colour. Thus a grey band cannot erase a stroke or turn its specks on/off.
fn ink(color: [u8; 3], field: u8, monochrome: bool) -> [u8; 3] {
    let [r, g, b] = color;
    let grain = (i16::from(field) - i16::from(luma(r, g, b))) * 3 / 8;
    let pale = field < CROSSOVER;
    let value = if pale {
        (252 - i16::from(field) / 12 + grain).clamp(195, 255) as u8
    } else {
        (5 + i16::from(255 - field) / 11 + grain).clamp(0, 58) as u8
    };
    if monochrome {
        return [value; 3];
    }
    if pale {
        let high = r.max(g).max(b);
        color.map(|c| value - (high - c) / 3)
    } else {
        let low = r.min(g).min(b);
        color.map(|c| value + (c - low) / 3)
    }
}

struct Exposure {
    columns: usize,
    values: Vec<u8>,
    samples: Vec<u8>,
}

impl Exposure {
    fn new(w: usize, h: usize) -> Self {
        let columns = (w - 1) / PITCH + 2;
        let rows = (h - 1) / PITCH + 2;
        Self {
            columns,
            values: vec![0; columns * rows],
            samples: vec![0; columns * rows],
        }
    }

    fn update(
        &mut self,
        bytes: &[u8],
        stride: usize,
        (sw, sh): (usize, usize),
        xs: &[usize],
        ys: &[usize],
        continuous: bool,
    ) {
        let probes = [-5, -2, 2, 5];
        for (j, row) in self.samples.chunks_exact_mut(self.columns).enumerate() {
            let y = ys[(j * PITCH).min(ys.len() - 1)] / (sw * stride);
            let rows = probes.map(|d| y.saturating_add_signed(d).min(sh - 1) * sw * stride);
            for (i, sample) in row.iter_mut().enumerate() {
                let x = xs[(i * PITCH).min(xs.len() - 1)] / stride;
                let cols = probes.map(|d| x.saturating_add_signed(d).min(sw - 1) * stride);
                let mut sum = 0u16;
                for row in rows {
                    for col in cols {
                        let p = row + col;
                        sum += u16::from(luma(bytes[p], bytes[p + 1], bytes[p + 2]));
                    }
                }
                *sample = ((sum + 8) / 16) as u8;
            }
        }
        // A single cluster of coarse samples must not punch white holes through
        // a dark stroke. Filter only the small guide, never the moving grain.
        let last_row = self.samples.len() / self.columns - 1;
        for (j, row) in self.values.chunks_exact_mut(self.columns).enumerate() {
            for (i, previous) in row.iter_mut().enumerate() {
                let left = i.saturating_sub(1);
                let right = (i + 1).min(self.columns - 1);
                let across = |y: usize| {
                    let line = &self.samples[y * self.columns..];
                    u16::from(line[left]) + 2 * u16::from(line[i]) + u16::from(line[right])
                };
                let tone = ((across(j.saturating_sub(1))
                    + 2 * across(j)
                    + across((j + 1).min(last_row))
                    + 8)
                    / 16) as u8;
                *previous = if continuous {
                    (*previous).clamp(tone.saturating_sub(DEADBAND), tone.saturating_add(DEADBAND))
                } else {
                    tone
                };
            }
        }
    }

    // Interpolate a continuous contour across cells; a tile never changes ink
    // as a block. Resolve the vertical component just once per occupied row.
    fn row(&self, y: usize, result: &mut [u16]) {
        let start = (y / PITCH) * self.columns;
        let dy = (y % PITCH) as u16;
        for (i, out) in result.iter_mut().enumerate() {
            *out = (PITCH as u16 - dy) * u16::from(self.values[start + i])
                + dy * u16::from(self.values[start + self.columns + i]);
        }
    }

    fn at(row: &[u16], x: usize) -> u8 {
        let i = x / PITCH;
        let dx = (x % PITCH) as u16;
        (((PITCH as u16 - dx) * row[i] + dx * row[i + 1] + 128) >> 8) as u8
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
    geometry: [usize; 10],
    xs: Vec<usize>,
    ys: Vec<usize>,
    exposure: Exposure,
    exposure_row: Vec<u16>,
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
            geometry: [0; 10],
            xs: Vec::with_capacity(w),
            ys: Vec::with_capacity(h),
            exposure: Exposure::new(w, h),
            exposure_row: vec![0; (w - 1) / PITCH + 2],
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
        monochrome: bool,
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
            usize::from(monochrome),
        ];
        let continuous = geometry == self.geometry;
        if !continuous {
            self.xs.clear();
            self.ys.clear();
            self.xs
                .extend(indices(rect.x, rect.w, w, rect.window_w, source.0).map(|x| x * stride));
            self.ys.extend(
                indices(rect.y, rect.h, h, rect.window_h, source.1).map(|y| y * source.0 * stride),
            );
            self.geometry = geometry;
        }
        self.exposure
            .update(bytes, stride, source, &self.xs, &self.ys, continuous);
        let mut row = usize::MAX;
        for run in &self.runs {
            if row != run.row {
                row = run.row;
                self.exposure.row(row, &mut self.exposure_row);
            }
            let src = &bytes[self.ys[run.row]..];
            let line = &mut output[run.row * w + run.start..run.row * w + run.end];
            for (x, px) in (run.start..run.end).zip(line) {
                let p = self.xs[x];
                let ink = ink(
                    [src[p], src[p + 1], src[p + 2]],
                    Exposure::at(&self.exposure_row, x),
                    monochrome,
                );
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
    if let Some(rgb) = frame.to_rgb8().filter(|p| p.width() > 0 && p.height() > 0) {
        return stamp.paint(
            rgb.as_bytes(),
            3,
            (rgb.width() as usize, rgb.height() as usize),
            &rect,
            monochrome,
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
                monochrome,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(gray: u8) -> f32 {
        let s = f32::from(gray) / 255.;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }

    #[test]
    fn every_uniform_gray_keeps_a_readable_solid_stroke_from_either_direction() {
        for gray in 0u8..=255 {
            for field in gray.saturating_sub(DEADBAND)..=gray.saturating_add(DEADBAND) {
                for mono in [false, true] {
                    let rgb = ink([gray; 3], field, mono);
                    assert_eq!([rgb[0]; 3], rgb);
                    let (fg, bg) = (luminance(rgb[0]), luminance(gray));
                    assert!(
                        (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05) >= 3.5,
                        "field {field}, grey {gray}, ink {rgb:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_sparse_grain_cluster_does_not_punch_a_white_hole_in_a_dark_stroke() {
        let side = 80;
        let mut pixels = vec![124; side * side * 3];
        for y in [27, 30, 34, 37] {
            for x in [27, 30, 34, 37] {
                let p = 3 * (y * side + x);
                pixels[p..p + 3].fill(100);
            }
        }
        let xs: Vec<_> = (0..side).map(|i| i * 3).collect();
        let ys: Vec<_> = xs.iter().map(|i| i * side).collect();
        let mut guide = Exposure::new(side, side);
        guide.update(&pixels, 3, (side, side), &xs, &ys, false);
        assert!(guide.values.iter().all(|&field| field >= CROSSOVER));
    }

    #[test]
    fn fine_grain_stays_in_register_while_the_field_passes_middle_gray() {
        let side = 96;
        let xs: Vec<_> = (0..side).map(|i| i * 3).collect();
        let ys: Vec<_> = xs.iter().map(|i| i * side).collect();
        let mut pixels = vec![0; 3 * side * side];
        let mut guide = Exposure::new(side, side);
        let mut row = vec![0; guide.columns];
        let mut seen = Vec::new();
        let mut prior = None;
        // Reverse the phase of 48-level grain on successive frames. A return
        // trip through the switching band should produce just two changes.
        let excursion = [111, 115, 117, 121, 116, 120, 127, 122, 117, 114, 120, 107];
        for (i, base) in excursion.into_iter().enumerate() {
            for (n, p) in pixels.chunks_exact_mut(3).enumerate() {
                let bright = (n % side + n / side + i) % 2 == 0;
                p.fill(if bright { base + 24 } else { base - 24 });
            }
            guide.update(&pixels, 3, (side, side), &xs, &ys, i > 0);
            for y in 32..64 {
                guide.row(y, &mut row);
                let field = Exposure::at(&row, 32);
                for x in 32..64 {
                    assert_eq!(Exposure::at(&row, x), field);
                }
                let low = ink([base - 24; 3], field, true);
                let high = ink([base + 24; 3], field, true);
                assert!(low[0] >= high[0] + 10);
                if y == 32 && prior != Some(field < CROSSOVER) {
                    seen.push(i);
                    prior = Some(field < CROSSOVER);
                }
            }
        }
        assert_eq!(seen, vec![0, 6, 11]);
    }
}
