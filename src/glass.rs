//! A small frosted lens above the view selector. Read the existing first-row
//! page tiles, filter in logical coordinates, then shade only the half-disc.
use crate::{GlassView, PageTile};
use slint::{Color, Image, Model, ModelRc, Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer};

type Rgb = [f32; 3];
const MARGIN: f32 = 6.0;
const PITCH: f32 = 3.5;

#[cfg(feature = "qa")]
thread_local! {
    pub(crate) static PROFILE: std::cell::Cell<(u32,u128)> = const { std::cell::Cell::new((0,0)) };
}

fn rgb(color: Color) -> Rgb {
    [color.red(), color.green(), color.blue()].map(f32::from)
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    std::array::from_fn(|i| a[i] + t * (b[i] - a[i]))
}

struct Tile {
    pixels: SharedPixelBuffer<Rgb8Pixel>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Tile {
    fn at(view: &GlassView, page: PageTile, col: usize) -> Option<Self> {
        let pixels = page.source.to_rgb8()?;
        let cell = view.viewport_w / view.columns.max(1) as f32;
        let w = cell - 2.0 * view.inset;
        let h = view.viewport_h / view.rows.max(1) as f32;
        if w <= 0.0 || page.w <= 0.0 || page.h <= 0.0 || pixels.as_slice().is_empty() {
            return None;
        }
        let fit = (w / page.w).min(h / page.h);
        let scale = if view.fit && (fit - 1.0).abs() > 0.006 {
            fit
        } else {
            1.0
        };
        let (width, height) = (page.w * scale, page.h * scale);
        // Same spine alignment and temporary fit as PagePane during a PDF resize.
        let offset = if !view.flow && view.columns == 2 {
            if col == 0 { w - width } else { 0.0 }
        } else {
            (w - width) * 0.5
        };
        Some(Self {
            pixels,
            width,
            height,
            x: col as f32 * cell + view.inset + offset,
            y: ((h - height) * 0.5).max(0.0),
        })
    }

    fn sample(&self, x: f32, y: f32) -> Option<Rgb> {
        let (u, v) = ((x - self.x) / self.width, (y - self.y) / self.height);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return None;
        }
        let (w, h) = (self.pixels.width() as usize, self.pixels.height() as usize);
        let p = self.pixels.as_slice()
            [((v * h as f32) as usize).min(h - 1) * w + ((u * w as f32) as usize).min(w - 1)];
        Some([p.r, p.g, p.b].map(f32::from))
    }
}

struct Frost {
    data: Vec<Rgb>,
    cols: usize,
    rows: usize,
    left: f32,
}

impl Frost {
    fn new(view: &GlassView, pages: &ModelRc<PageTile>) -> Self {
        // The lens ends within the first grid row, including at the minimum window height.
        let tiles: [Option<Tile>; 4] = std::array::from_fn(|i| {
            if i >= view.columns as usize {
                return None;
            }
            Tile::at(view, pages.row_data(i)?, i)
        });
        let paper = rgb(view.paper);
        let chrome = rgb(view.chrome);
        let cell = view.viewport_w / view.columns.max(1) as f32;
        let sample = |x: f32, y: f32| {
            let i = (x / cell).floor() as usize;
            let local = x - i as f32 * cell;
            if local < view.inset || local >= cell - view.inset {
                return paper;
            }
            tiles
                .get(i)
                .and_then(Option::as_ref)
                .and_then(|t| t.sample(x, y.max(0.0)))
                .unwrap_or(paper)
        };
        let left = -view.radius - MARGIN;
        let cols = ((-left * 2.0) / PITCH).ceil() as usize + 2;
        let rows = ((view.radius + MARGIN) / PITCH).ceil() as usize + 2;
        let mut data = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            let y = row as f32 * PITCH;
            // Light from the rail spreads into the glass on either side of the crossing.
            let rail = ((view.rail + 4.5 - y) / 9.0).clamp(0.0, 1.0);
            for col in 0..cols {
                let x = view.center + left + col as f32 * PITCH;
                let mut sum = [0.; 3];
                for (dx, dy) in [(-0.9, -0.9), (0.9, -0.9), (-0.9, 0.9), (0.9, 0.9)] {
                    let point = sample(x + dx, y + dy);
                    for c in 0..3 {
                        sum[c] += point[c];
                    }
                }
                data.push(mix(sum.map(|v| v * 0.25), chrome, rail * 0.72));
            }
        }
        // A separable binomial filter integrates adjacent cells rather than
        // aliasing thin PDF rules into a spotted pattern under the glass.
        let mut scratch = vec![[0.; 3]; data.len()];
        for horizontal in [true, false] {
            for (i, dst) in scratch.iter_mut().enumerate() {
                let (x, y) = (i % cols, i / cols);
                let mut sum = [0.; 3];
                for (offset, weight) in [(-2, 1.), (-1, 4.), (0, 6.), (1, 4.), (2, 1.)] {
                    let j = if horizontal {
                        y * cols + x.saturating_add_signed(offset).min(cols - 1)
                    } else {
                        y.saturating_add_signed(offset).min(rows - 1) * cols + x
                    };
                    for c in 0..3 {
                        sum[c] += weight * data[j][c];
                    }
                }
                *dst = sum.map(|v| v / 16.);
            }
            std::mem::swap(&mut scratch, &mut data);
        }
        let mut field = Self {
            data,
            cols,
            rows,
            left,
        };
        let wash = if view.dark {
            [29., 43., 47.]
        } else {
            [218., 232., 230.]
        };
        for (i, dst) in scratch.iter_mut().enumerate() {
            let x = left + (i % cols) as f32 * PITCH;
            let y = (i / cols) as f32 * PITCH;
            let nx = x / view.radius;
            let ny = y / view.radius;
            let radial = (nx * nx + ny * ny).min(1.);
            let bend = 0.966 - 0.16 * radial.powi(3);
            let src = field.sample(x * bend, y * bend);
            let l = (src[0] * 0.2126 + src[1] * 0.7152 + src[2] * 0.0722) / 255.;
            let tint = if view.dark {
                0.40 + 0.33 * l
            } else {
                0.35 + 0.31 * (1. - l)
            };
            let base = mix(src, wash, tint);
            let broad = (1. - ((nx + 0.59).powi(2) * 1.75 + (ny - 0.20).powi(2) * 3.1)).max(0.);
            // The curved grazing reflection has one light source for both themes.
            let sweep = (1. - ((ny + 0.24 * nx - 0.14 * nx * nx - 0.18).abs() / 0.25)).max(0.);
            let gleam = broad * broad * (if view.dark { 0.091 } else { 0.16 })
                + sweep * sweep * broad * 0.045;
            *dst = mix(base, [248., 254., 253.], gleam);
        }
        field.data = scratch;
        field
    }

    fn sample(&self, x: f32, y: f32) -> Rgb {
        let u = ((x - self.left) / PITCH).clamp(0., (self.cols - 2) as f32);
        let v = (y / PITCH).clamp(0., (self.rows - 2) as f32);
        let idx = v as usize * self.cols + u as usize;
        let a = mix(self.data[idx], self.data[idx + 1], u.fract());
        let b = mix(
            self.data[idx + self.cols],
            self.data[idx + self.cols + 1],
            u.fract(),
        );
        mix(a, b, v.fract())
    }
}

pub(crate) fn draw(view: GlassView, pages: ModelRc<PageTile>) -> Image {
    #[cfg(feature = "qa")]
    let started = std::time::Instant::now();
    let r = view.radius.clamp(1., 100.);
    let frost = Frost::new(&view, &pages);
    // The core is low frequency. Cap raster density at 1.5× and antialias
    // the analytic rim; larger desktop scales interpolate this small surface.
    let half = ((r + MARGIN) * view.density.clamp(1., 1.5)).ceil() as u32;
    let step = (r + MARGIN) / half as f32;
    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(2 * half, half);
    let mut scanline = vec![[0.; 3]; frost.cols];
    let rim = if view.dark {
        [108., 150., 151.]
    } else {
        [79., 110., 116.]
    };
    let white = [248., 254., 253.];
    for (row, line) in buffer
        .make_mut_bytes()
        .chunks_exact_mut(half as usize * 8)
        .enumerate()
    {
        let y = (row as f32 + 0.5) * step;
        let v = y / PITCH;
        let top = v as usize * frost.cols;
        for (i, p) in scanline.iter_mut().enumerate() {
            *p = mix(
                frost.data[top + i],
                frost.data[top + frost.cols + i],
                v.fract(),
            );
        }
        for (i, px) in line.chunks_exact_mut(4).enumerate() {
            let offset = (i as f32 + 0.5) * step;
            let x = offset - (r + MARGIN);
            let squared = x * x + y * y;
            if squared > (r + MARGIN).powi(2) {
                continue;
            }
            let u = offset / PITCH;
            let mut body = mix(scanline[u as usize], scanline[u as usize + 1], u.fract());
            if squared < (r - 5.).powi(2) {
                for c in 0..3 {
                    px[c] = (body[c] + 0.5) as u8;
                }
                px[3] = 255;
                continue;
            }
            let dist = squared.sqrt();
            let edge = r - dist;
            let cover = (edge / step + 0.5).clamp(0., 1.);
            let shade = if cover >= 1.0 {
                0.0
            } else {
                (1.0 - (((x * x + (y - 1.5) * (y - 1.5)).sqrt() - r).max(0.) / 4.)).clamp(0., 1.)
            };
            let shadow_alpha = shade * shade * if view.dark { 0.43 } else { 0.18 };
            let alpha = cover + (1. - cover) * shadow_alpha;
            if alpha < 0.003 {
                continue;
            }
            let mut light = [12., 23., 25.];
            if cover > 0. {
                let incidence = ((-0.82 * x + 0.57 * y) / dist.max(0.5)).clamp(-1., 1.);
                let narrow = (1. - ((edge - 0.85).abs() / 1.25)).max(0.);
                let glint = (1. - ((edge - 3.0).abs() / 2.2)).max(0.);
                body = mix(body, rim, 0.47 * (1. - (edge.max(0.) / 5.).min(1.)).powi(2));
                body = mix(
                    body,
                    white,
                    narrow * (0.32 + 0.60 * incidence.max(0.).powi(2))
                        + glint * 0.25 * (-incidence).max(0.),
                );
                light = mix(light, body, cover / alpha);
            }
            for c in 0..3 {
                px[c] = (light[c] + 0.5) as u8;
            }
            px[3] = (alpha * 255. + 0.5) as u8;
        }
    }
    #[cfg(feature = "qa")]
    PROFILE.with(|counter| {
        let (calls, us) = counter.get();
        counter.set((calls + 1, us + started.elapsed().as_micros()));
    });
    Image::from_rgba8(buffer)
}
