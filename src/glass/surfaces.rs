//! A single low-frequency field serves all visible reading surfaces. No desktop
//! capture, window-sized blur, retained document copy or animation timer.
use super::{PageSamples, mix};
use crate::{FrostKind, FrostMap, FrostRect, PageScene, PageTile};
use slint::{Image, ModelRc, Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer};

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
    if map.w < 1. || map.h < 1. || area.w < 1. || area.h < 1. {
        return Image::default();
    }
    let Some(field) = map.picture.to_rgb8() else {
        return Image::default();
    };
    let (fw, fh) = (field.width() as usize, field.height() as usize);
    if fw == 0 || fh == 0 {
        return Image::default();
    }
    let data = field.as_slice();
    let step = (area.w / 376.).max(area.h / 214.).max(2.75);
    let (w, h) = (
        (area.w / step).ceil().max(1.) as usize,
        (area.h / step).ceil().max(1.) as usize,
    );
    let (sx, sy) = (area.w / w as f32, area.h / h as f32);
    let transmission = match kind {
        FrostKind::Rail => (0.34, 0.23),
        FrostKind::Dock => (0.32, 0.22),
        FrostKind::Panel => (0.23, 0.16),
        FrostKind::Menu => (0.13, 0.10),
        FrostKind::Notice => (0.19, 0.14),
    };
    let t = if map.dark {
        transmission.1
    } else {
        transmission.0
    };
    let base = if map.dark {
        [32., 43., 42.]
    } else {
        [246., 248., 242.]
    };
    let lift = if map.dark {
        [161., 183., 181.]
    } else {
        [255., 255., 251.]
    };
    let read = |x: usize, y: usize| {
        let p = data[y * fw + x];
        [p.r, p.g, p.b].map(f32::from)
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
            let u = ((area.x + x) / map.w * fw as f32 - 0.5).clamp(0., (fw - 1) as f32);
            let v = ((area.y + y) / map.h * fh as f32 - 0.5).clamp(0., (fh - 1) as f32);
            let (a, b) = (u as usize, v as usize);
            let (c, d) = ((a + 1).min(fw - 1), (b + 1).min(fh - 1));
            let paper = mix(
                mix(read(a, b), read(c, b), u.fract()),
                mix(read(a, d), read(c, d), u.fract()),
                v.fract(),
            );
            let grazing = (1. - y / 24.).clamp(0., 1.);
            let light = (0.015 + 0.046 * grazing * grazing) * (1. - 0.45 * x / area.w);
            // Let coloured ink tint the glass. Black or white paper should
            // change its exposure gently across the edge of the sheet.
            let [r, g, b] = paper;
            let chroma = (r.max(g).max(b) - r.min(g).min(b)) / 64.;
            let transmission = t * (0.35 + 0.65 * chroma.min(1.));
            let color = mix(mix(base, paper, transmission), lift, light);
            let alpha = fade * fade * (3. - 2. * fade);
            [px.r, px.g, px.b] = color.map(|v| (v * alpha + 0.5) as u8);
            px.a = (alpha * 255. + 0.5) as u8;
        }
    }
    #[cfg(feature = "qa")]
    COST.with(|p| {
        let (n, f, t, s) = p.get();
        p.set((n, f + 1, t, s + started.elapsed().as_micros()));
    });
    Image::from_rgba8_premultiplied(out)
}
