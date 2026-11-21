//! A single low-frequency field serves all visible reading surfaces. No desktop
//! capture, window-sized blur, retained document copy or animation timer.
use super::{PageSamples, Rgb, mix};
use crate::{FrostKind, FrostMap, FrostRect, PageScene, PageTile};
use slint::{Color, Image, ModelRc, Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer};

#[cfg(feature = "qa")]
thread_local! {
    pub(crate) static COST: std::cell::Cell<(u32,u32,u128,u128)> = const { std::cell::Cell::new((0,0,0,0)) };
}

pub(crate) fn backdrop(scene: PageScene, pages: ModelRc<PageTile>) -> Image {
    #[cfg(feature = "qa")]
    let started = std::time::Instant::now();
    let (w, h) = (scene.viewport_w, scene.viewport_h);
    if w < 1. || h < 1. {
        return Image::default();
    }
    let pitch = (w / 144.).max(h / 100.).max(11.);
    let (cols, rows) = (
        (w / pitch).ceil().max(2.) as usize,
        (h / pitch).ceil().max(2.) as usize,
    );
    let (sx, sy) = (w / cols as f32, h / rows as f32);
    let scene = PageSamples::new(&scene, &pages, false);
    let mut field = SharedPixelBuffer::<Rgb8Pixel>::new(cols as u32, rows as u32);
    for (y, line) in field.make_mut_slice().chunks_exact_mut(cols).enumerate() {
        for (x, px) in line.iter_mut().enumerate() {
            let mut sum = [0.; 3];
            // Stratification integrates stems and narrow rules before downsampling.
            for j in 0..3 {
                for i in 0..3 {
                    let fx = (i as f32 + 0.25 + 0.17 * ((i + j) % 3) as f32) / 3.;
                    let fy = (j as f32 + 0.25 + 0.17 * ((i + 2 * j) % 3) as f32) / 3.;
                    let point = scene.at((x as f32 + fx) * sx, (y as f32 + fy) * sy);
                    for c in 0..3 {
                        sum[c] += point[c];
                    }
                }
            }
            [px.r, px.g, px.b] = sum.map(|v| (v / 9.).round() as u8);
        }
    }
    let mut scratch = SharedPixelBuffer::<Rgb8Pixel>::new(cols as u32, rows as u32);
    for horizontal in [true, false] {
        let src = field.as_bytes();
        let dst = scratch.make_mut_bytes();
        for y in 0..rows {
            for x in 0..cols {
                let at = |offset: isize| {
                    if horizontal {
                        (y * cols + x.saturating_add_signed(offset).min(cols - 1)) * 3
                    } else {
                        (y.saturating_add_signed(offset).min(rows - 1) * cols + x) * 3
                    }
                };
                let [a, b, m, d, e] = [-2, -1, 0, 1, 2].map(at);
                for c in 0..3 {
                    let value = u16::from(src[a + c])
                        + 4 * u16::from(src[b + c])
                        + 6 * u16::from(src[m + c])
                        + 4 * u16::from(src[d + c])
                        + u16::from(src[e + c]);
                    dst[m + c] = ((value + 8) / 16) as u8;
                }
            }
        }
        std::mem::swap(&mut field, &mut scratch);
    }
    #[cfg(feature = "qa")]
    COST.with(|p| {
        let (n, f, t, s) = p.get();
        p.set((n + 1, f, t + started.elapsed().as_micros(), s));
    });
    Image::from_rgb8(field)
}

// The atlas already contains the page blur. Read its shared RGB buffer in the
// same logical window coordinates for flat panels and for refracted samples.
struct PageField {
    pixels: SharedPixelBuffer<Rgb8Pixel>,
    cols: usize,
    rows: usize,
    scale_x: f32,
    scale_y: f32,
}

impl PageField {
    fn from_map(map: &FrostMap) -> Option<Self> {
        if map.w < 1. || map.h < 1. {
            return None;
        }
        let pixels = map.picture.to_rgb8()?;
        let (cols, rows) = (pixels.width() as usize, pixels.height() as usize);
        (cols > 0 && rows > 0).then(|| Self {
            pixels,
            cols,
            rows,
            scale_x: cols as f32 / map.w,
            scale_y: rows as f32 / map.h,
        })
    }

    fn at(&self, x: f32, y: f32) -> Rgb {
        let u = (x * self.scale_x - 0.5).clamp(0., (self.cols - 1) as f32);
        let v = (y * self.scale_y - 0.5).clamp(0., (self.rows - 1) as f32);
        let (x0, y0) = (u as usize, v as usize);
        let (x1, y1) = ((x0 + 1).min(self.cols - 1), (y0 + 1).min(self.rows - 1));
        let read = |col, row| {
            let p: Rgb8Pixel = self.pixels.as_slice()[row * self.cols + col];
            [p.r, p.g, p.b].map(f32::from)
        };
        mix(
            mix(read(x0, y0), read(x1, y0), u.fract()),
            mix(read(x0, y1), read(x1, y1), u.fract()),
            v.fract(),
        )
    }
}

fn base_tint(dark: bool) -> Rgb {
    if dark {
        [32., 43., 42.]
    } else {
        [246., 248., 242.]
    }
}

fn tint(base: Rgb, paper: Rgb, strength: f32) -> Rgb {
    let [r, g, b] = paper;
    let chroma = ((r.max(g).max(b) - r.min(g).min(b)) / 64.).min(1.);
    mix(base, paper, strength * (0.35 + 0.65 * chroma))
}

const DOCK_TRANSMISSION: (f32, f32) = (0.32, 0.22);

// Continue the dock tint to its native vector edge. The low-resolution face
// fades into this undercoat so its texels do not outline the shoulder.
pub(crate) fn dock_edge(map: FrostMap, x: f32, y: f32) -> Color {
    let base = base_tint(map.dark);
    let strength = if map.dark {
        DOCK_TRANSMISSION.1
    } else {
        DOCK_TRANSMISSION.0
    };
    let color =
        PageField::from_map(&map).map_or(base, |field| tint(base, field.at(x, y), strength));
    let [r, g, b] = color.map(|v| (v + 0.5) as u8);
    Color::from_rgb_u8(r, g, b)
}

#[cfg(feature = "qa")]
fn record_face(started: std::time::Instant) {
    COST.with(|p| {
        let (n, f, t, s) = p.get();
        p.set((n, f + 1, t, s + started.elapsed().as_micros()));
    });
}

// Keep the fine, continuously curved outline in Slint. Fade the sampled field
// into its material colour just inside that boundary, including the dock shoulder.
fn inset(x: f32, y: f32, w: f32, h: f32, corner: f32) -> f32 {
    let r = corner.min(w / 2.).min(h / 2.);
    let dx = (x - w / 2.).abs() - (w / 2. - r);
    let dy = (y - h / 2.).abs() - (h / 2. - r);
    if dx > 0. && dy > 0. {
        r - dx.hypot(dy)
    } else {
        r - dx.max(dy)
    }
}

pub(crate) fn panel(map: FrostMap, area: FrostRect, kind: FrostKind) -> Image {
    #[cfg(feature = "qa")]
    let started = std::time::Instant::now();
    if area.w < 1. || area.h < 1. {
        return Image::default();
    }
    let Some(field) = PageField::from_map(&map) else {
        return Image::default();
    };
    let step = (area.w / 376.).max(area.h / 214.).max(2.75);
    let (w, h) = (
        (area.w / step).ceil().max(1.) as usize,
        (area.h / step).ceil().max(1.) as usize,
    );
    let (sx, sy) = (area.w / w as f32, area.h / h as f32);
    let transmission = match kind {
        FrostKind::Rail => (0.34, 0.23),
        FrostKind::Dock => DOCK_TRANSMISSION,
        FrostKind::Panel => (0.23, 0.16),
        FrostKind::Menu => (0.13, 0.10),
        FrostKind::Notice => (0.19, 0.14),
    };
    let t = if map.dark {
        transmission.1
    } else {
        transmission.0
    };
    let base = base_tint(map.dark);
    let lift = if map.dark {
        [161., 183., 181.]
    } else {
        [255., 255., 251.]
    };
    let mut out = SharedPixelBuffer::<Rgba8Pixel>::new(w as u32, h as u32);
    for (row, line) in out.make_mut_slice().chunks_exact_mut(w).enumerate() {
        let y = (row as f32 + 0.5) * sy;
        for (col, px) in line.iter_mut().enumerate() {
            let x = (col as f32 + 0.5) * sx;
            let edge = match kind {
                FrostKind::Rail => 99.,
                FrostKind::Dock if area.rise > 0. => {
                    let bay = (area.w - 100.).max(0.);
                    inset(x, y - area.rise, area.w, area.h - area.rise, area.corner).max(inset(
                        x - bay,
                        y,
                        area.w - bay,
                        area.h,
                        area.corner,
                    ))
                }
                _ => inset(x, y, area.w, area.h, area.corner),
            };
            let fade = ((edge - 1.7) / 5.0).clamp(0., 1.);
            if fade <= 0. {
                continue;
            }
            let paper = field.at(area.x + x, area.y + y);
            let grazing = (1. - y / 24.).clamp(0., 1.);
            let light = (0.015 + 0.046 * grazing * grazing) * (1. - 0.45 * x / area.w);
            // Let coloured ink tint the glass. Black or white paper should
            // change its exposure gently across the edge of the sheet.
            let color = mix(tint(base, paper, t), lift, light);
            let alpha = fade * fade * (3. - 2. * fade);
            [px.r, px.g, px.b] = color.map(|v| (v * alpha + 0.5) as u8);
            px.a = (alpha * 255. + 0.5) as u8;
        }
    }
    #[cfg(feature = "qa")]
    record_face(started);
    Image::from_rgba8_premultiplied(out)
}

pub(crate) fn zoom_lens(map: FrostMap, cx: f32, cy: f32, radius: f32, density: f32) -> Image {
    #[cfg(feature = "qa")]
    let started = std::time::Instant::now();
    let Some(field) = PageField::from_map(&map) else {
        return Image::default();
    };
    let r = radius.clamp(1., 34.);
    // DialFace reserves 5% of its diameter per side for this contact shadow.
    let half = 1.1 * r;
    let size = (2. * half * density.clamp(1., 1.5)).ceil() as u32;
    let step = 2. * half / size as f32;
    let rim_width = (r * 0.08).max(1.4);
    let (wash, rim) = if map.dark {
        ([37., 50., 51.], [101., 131., 130.])
    } else {
        ([226., 236., 232.], [80., 108., 111.])
    };
    let mut out = SharedPixelBuffer::<Rgba8Pixel>::new(size, size);
    for (row, line) in out
        .make_mut_slice()
        .chunks_exact_mut(size as usize)
        .enumerate()
    {
        let y = (row as f32 + 0.5) * step - half;
        for (col, px) in line.iter_mut().enumerate() {
            let x = (col as f32 + 0.5) * step - half;
            let radius_sq = x * x + y * y;
            let edge = r - radius_sq.sqrt();
            let cover = (edge / step + 0.5).clamp(0., 1.);
            let contact = if cover < 1. {
                let below = (x * x + (y - 0.035 * r).powi(2)).sqrt() - r;
                (1. - below / (0.06 * r)).clamp(0., 1.).powi(2) * if map.dark { 0.40 } else { 0.22 }
            } else {
                0.
            };
            let shadow = (1. - cover) * contact;
            if cover + shadow == 0. {
                continue;
            }
            let mut body = [0.; 3];
            if cover > 0. {
                let (nx, ny) = (x / r, y / r);
                let bend = 0.97 - 0.26 * (radius_sq / (r * r)).min(1.).powi(3);
                let paper = field.at(cx + x * bend, cy + y * bend);
                let [red, green, blue] = paper;
                let chroma = ((red.max(green).max(blue) - red.min(green).min(blue)) / 90.).min(1.);
                let transmit = if map.dark {
                    0.09 + 0.16 * chroma
                } else {
                    // Keep dark, saturated ink from swallowing the readout.
                    let luma = (0.2126 * red + 0.7152 * green + 0.0722 * blue) / 255.;
                    0.10 + (0.25 + 0.09 * luma) * chroma
                };
                body = mix(wash, paper, transmit);
                // One broad reflection from upper left and a narrow meniscus;
                // the middle remains quiet behind the printed percentage.
                let shoulder = (1. - 1.7 * (nx + 0.43).powi(2) - 3.1 * (ny + 0.61).powi(2)).max(0.);
                let bevel = (1. - edge.max(0.) / rim_width).max(0.);
                let incidence = (-0.55 * nx - 0.83 * ny).clamp(-1., 1.);
                body = mix(body, rim, bevel * bevel * (0.59 - 0.10 * incidence));
                let ridge = (1. - (edge / rim_width - 0.38).abs() / 0.42).max(0.);
                let reflection = shoulder * shoulder * if map.dark { 0.07 } else { 0.15 }
                    + ridge * (0.11 + 0.73 * incidence.max(0.).powi(2));
                body = mix(body, [248., 254., 253.], reflection);
            }
            let shadow_ink = [13., 24., 26.];
            let pixel: Rgb = std::array::from_fn(|i| body[i] * cover + shadow_ink[i] * shadow);
            [px.r, px.g, px.b] = pixel.map(|v| (v + 0.5) as u8);
            px.a = ((cover + shadow) * 255. + 0.5) as u8;
        }
    }
    #[cfg(feature = "qa")]
    record_face(started);
    Image::from_rgba8_premultiplied(out)
}
