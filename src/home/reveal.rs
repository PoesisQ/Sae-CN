//! The last cover breaks up over the already rendered book. Each grain has one
//! release time for the whole exposure. No document pixels are read or copied.
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::{collections::hash_map::RandomState, hash::BuildHasher, time::Duration};

const STEPS: u8 = 9;
pub(super) const FRAME_INTERVAL: Duration = Duration::from_millis(30);
pub(super) const DURATION: Duration = FRAME_INTERVAL.saturating_mul(STEPS as u32);

pub(super) struct Dissolve {
    current: Image,
    step: u8,
    seed: u32,
    grains: Option<Grains>,
}

impl Dissolve {
    pub(super) fn new(cover: &Image) -> Option<Self> {
        let size = cover.size();
        (size.width != 0 && size.height != 0).then(|| Self {
            current: cover.clone(),
            step: 0,
            seed: RandomState::new().hash_one(0u8) as u32,
            grains: None,
        })
    }

    pub(super) fn frame(&mut self, elapsed: Duration) -> Option<Image> {
        if elapsed >= DURATION || self.step == STEPS {
            self.current = Image::default();
            self.grains = None;
            self.step = STEPS;
            return None;
        }
        let step = (elapsed.as_millis() / FRAME_INTERVAL.as_millis()) as u8;
        if step > self.step {
            // Leave the first book paint free. A missed timer beat jumps directly
            // to the current exposure rather than rendering intermediate frames.
            if self.grains.is_none() {
                self.grains = Grains::capture(&self.current, self.seed);
            }
            self.current = self.grains.as_mut()?.expose(step);
            self.step = step;
        }
        Some(self.current.clone())
    }
}

struct Grain {
    color: [u8; 3],
    order: u8,
}

struct Grains {
    width: u32,
    height: u32,
    print: Vec<Grain>,
    buffers: [Option<SharedPixelBuffer<Rgba8Pixel>>; 2],
    next: usize,
}

fn hash(mut n: u32) -> u8 {
    n = (n ^ (n >> 16)).wrapping_mul(0x7feb_352d);
    n = (n ^ (n >> 15)).wrapping_mul(0x846c_a68b);
    ((n ^ (n >> 16)) >> 24) as u8
}

fn smooth(x: f32) -> f32 {
    x * x * (3. - 2. * x)
}

fn luma([r, g, b]: [u8; 3]) -> u16 {
    (54 * u16::from(r) + 183 * u16::from(g) + 19 * u16::from(b) + 128) >> 8
}

impl Grains {
    fn dimensions(sw: u32, sh: u32) -> (usize, usize) {
        let scale = (576. / sw as f32).min(360. / sh as f32).min(1.);
        (
            (sw as f32 * scale).round().max(1.) as usize,
            (sh as f32 * scale).round().max(1.) as usize,
        )
    }

    fn capture(cover: &Image, seed: u32) -> Option<Self> {
        if let Some(p) = cover.to_rgb8() {
            return Some(Self::sample(p.as_bytes(), p.width(), p.height(), 3, seed));
        }
        // Before the first animation tick the opaque preview can be RGBA.
        let p = cover.to_rgba8_premultiplied()?;
        Some(Self::sample(p.as_bytes(), p.width(), p.height(), 4, seed))
    }

    fn sample(source: &[u8], sw: u32, sh: u32, channels: usize, seed: u32) -> Self {
        let (w, h) = Self::dimensions(sw, sh);
        let columns: Vec<_> = (0..w)
            .map(|x| ((2 * x + 1) * sw as usize / (2 * w)) * channels)
            .collect();
        let mut print = Vec::with_capacity(w * h);
        for y in 0..h {
            let sy = (2 * y + 1) * sh as usize / (2 * h);
            let row = &source[sy * sw as usize * channels..];
            for &sx in &columns {
                let color = [row[sx], row[sx + 1], row[sx + 2]];
                // Each texel gets an independent time and lifetime. Its ink,
                // neighbours and position within the page have no preference.
                let order = hash(print.len() as u32 ^ seed);
                print.push(Grain { color, order });
            }
        }
        Self {
            width: w as u32,
            height: h as u32,
            print,
            buffers: [None, None],
            next: 0,
        }
    }

    fn expose(&mut self, step: u8) -> Image {
        let t = f32::from(step) / f32::from(STEPS);
        let elapsed = t * (1.28 - 0.28 * t);
        let exposure: [(u8, u16); 256] = std::array::from_fn(|order| {
            let release = ((order >> 2) as f32 + 0.5) / 64.;
            let lifetime = 0.08 + (order & 3) as f32 * 0.015;
            let a = smooth(((release - elapsed) / lifetime + 0.5).clamp(0., 1.));
            let alpha = (255. * a).round() as u8;
            // Only an individual grain losing opacity can pick up a cool edge.
            let tint = 0.22 * (1. - a);
            (alpha, (f32::from(alpha) * tint).round() as u16)
        });
        let buffer = self.buffers[self.next]
            .get_or_insert_with(|| SharedPixelBuffer::<Rgba8Pixel>::new(self.width, self.height));
        for (pixel, grain) in buffer.make_mut_slice().iter_mut().zip(&self.print) {
            let (alpha, blue) = exposure[usize::from(grain.order)];
            pixel.a = alpha;
            if alpha == 0 {
                [pixel.r, pixel.g, pixel.b] = [0; 3];
            } else if alpha == 255 && blue == 0 {
                [pixel.r, pixel.g, pixel.b] = grain.color;
            } else {
                let light = luma(grain.color);
                let silver_blue = [
                    12 + (light * 151 >> 8),
                    38 + (light * 163 >> 8),
                    102 + (light * 144 >> 8),
                ];
                let keep = u16::from(alpha) - blue;
                [pixel.r, pixel.g, pixel.b] = std::array::from_fn(|c| {
                    ((u16::from(grain.color[c]) * keep + silver_blue[c] * blue + 127) / 255) as u8
                });
            }
        }
        let image = Image::from_rgba8_premultiplied(buffer.clone());
        self.next ^= 1;
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Rgb8Pixel;

    #[test]
    fn cover_grains_reveal_the_page_monotonically_and_release_their_buffers() {
        let mut pixels = SharedPixelBuffer::<Rgb8Pixel>::new(128, 80);
        for (i, p) in pixels.make_mut_slice().iter_mut().enumerate() {
            let tone = (i / 128 * 3) as u8;
            [p.r, p.g, p.b] = [tone / 3, tone / 2, tone];
        }
        let start = Image::from_rgb8(pixels);
        let mut reveal = Dissolve::new(&start).unwrap();
        assert_eq!(reveal.frame(Duration::ZERO), Some(start));
        assert!(reveal.grains.is_none());
        let mut previous = vec![255; 128 * 80];
        for step in 1..STEPS {
            let time = FRAME_INTERVAL * u32::from(step);
            let frame = reveal.frame(time).unwrap();
            assert_eq!(
                reveal.frame(time + Duration::from_millis(5)),
                Some(frame.clone())
            );
            let rgba = frame.to_rgba8_premultiplied().unwrap();
            for (last, p) in previous.iter_mut().zip(rgba.as_slice()) {
                assert!(p.a <= *last && p.r <= p.a && p.g <= p.a && p.b <= p.a);
                *last = p.a;
            }
            if step == 4 {
                let clear = previous.iter().filter(|&&a| a == 0).count();
                let solid = previous.iter().filter(|&&a| a == 255).count();
                assert!(clear > previous.len() / 4 && solid > previous.len() / 4);
            }
        }
        assert!(reveal.frame(DURATION).is_none());
        assert!(reveal.grains.is_none() && reveal.current.size().width == 0);
        assert!(reveal.frame(Duration::ZERO).is_none());
    }

    #[test]
    fn every_region_starts_scattering_together_regardless_of_its_ink() {
        const W: usize = 192;
        const H: usize = 128;
        let light_to_dark: Vec<u8> = (0..W * H)
            .flat_map(|i| {
                let ink = (i / W * 2) as u8;
                [ink, ink / 2, 255 - ink]
            })
            .collect();
        let negative: Vec<_> = light_to_dark.iter().map(|&v| 255 - v).collect();
        for seed in [0x9415_6229, 0x2a37_84f9] {
            let capture = |ink: &[u8]| Grains::sample(ink, W as u32, H as u32, 3, seed);
            let mut sheet = capture(&light_to_dark);
            assert!(
                sheet
                    .print
                    .iter()
                    .zip(capture(&negative).print)
                    .all(|(a, b)| a.order == b.order)
            );
            for (step, minimum, maximum) in [(1, 0.045, 0.14), (4, 0.40, 0.52), (8, 0.80, 0.93)] {
                let frame = sheet.expose(step).to_rgba8_premultiplied().unwrap();
                let mut clear = [0u32; 16];
                let mut solid = [0u32; 16];
                for (i, p) in frame.as_slice().iter().enumerate() {
                    let tile = (i / W / (H / 4)) * 4 + i % W / (W / 4);
                    clear[tile] += u32::from(p.a == 0);
                    solid[tile] += u32::from(p.a == 255);
                }
                for (clear, solid) in clear.into_iter().zip(solid) {
                    let fraction = (clear * 16) as f32 / (W * H) as f32;
                    assert!(
                        fraction > minimum && fraction < maximum && solid > 8,
                        "{step}: {fraction}, {solid}"
                    );
                }
            }
        }
    }
}
