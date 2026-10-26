use crate::document::Book;
use anyhow::{Result, ensure};
use mupdf::{IRect, Matrix, TextPageFlags, text_page::TextBlockType};
use std::{collections::VecDeque, sync::Arc, time::Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Bounds {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}
pub fn quad_rect(q: &mupdf::Quad) -> mupdf::Rect {
    mupdf::Rect::new(
        q.ul.x.min(q.ur.x).min(q.ll.x).min(q.lr.x),
        q.ul.y.min(q.ur.y).min(q.ll.y).min(q.lr.y),
        q.ul.x.max(q.ur.x).max(q.ll.x).max(q.lr.x),
        q.ul.y.max(q.ur.y).max(q.ll.y).max(q.lr.y),
    )
}
#[derive(Clone, Debug)]
pub struct Glyph {
    pub ch: char,
    pub bounds: Bounds,
}
#[derive(Clone, Debug)]
pub struct PageLink {
    pub bounds: Bounds,
    pub uri: String,
}
/// One RGB allocation shared by the worker cache and Slint's software renderer.
pub struct Pixels {
    #[cfg(feature = "native-ui")]
    buffer: slint::SharedPixelBuffer<slint::Rgb8Pixel>,
    #[cfg(not(feature = "native-ui"))]
    buffer: Vec<u8>,
}
impl Pixels {
    fn new(bytes: &[u8], width: u32, height: u32) -> Self {
        #[cfg(feature = "native-ui")]
        let buffer = slint::SharedPixelBuffer::clone_from_slice(bytes, width, height);
        #[cfg(not(feature = "native-ui"))]
        let buffer = {
            let _ = (width, height);
            bytes.to_vec()
        };
        Self { buffer }
    }
    #[cfg(feature = "native-ui")]
    pub fn image(&self) -> slint::Image {
        slint::Image::from_rgb8(self.buffer.clone())
    }
}
impl std::ops::Deref for Pixels {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        #[cfg(feature = "native-ui")]
        {
            self.buffer.as_bytes()
        }
        #[cfg(not(feature = "native-ui"))]
        {
            &self.buffer
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct RenderView {
    pub viewport: Option<(u32, u32, [f32; 2])>,
    pub tint: Option<(i32, i32)>,
    pub invert: bool,
}

pub struct Frame {
    pub pixels: Pixels,
    pub width: u32,
    pub height: u32,
    pub display_width: f32,
    pub display_height: f32,
    pub page: i32,
    pub glyphs: Vec<Glyph>,
    pub links: Vec<PageLink>,
    pub images: Vec<Bounds>,
    pub scale: f32,
    pub dpi: f32,
    pub origin: (f32, f32),
    pub overflow: (f32, f32),
    pub render_ms: f64,
}
impl Frame {
    pub fn cost(&self) -> usize {
        self.pixels.len()
            + self.glyphs.capacity() * std::mem::size_of::<Glyph>()
            + self.images.capacity() * std::mem::size_of::<Bounds>()
            + self.links.capacity() * std::mem::size_of::<PageLink>()
            + self.links.iter().map(|l| l.uri.capacity()).sum::<usize>()
    }
    pub fn select(&self, a: (f32, f32), b: (f32, f32)) -> (String, Vec<Bounds>) {
        let nearest = |p: (f32, f32)| {
            self.glyphs
                .iter()
                .enumerate()
                .filter(|(_, g)| g.ch != '\n')
                .min_by(|(_, a), (_, b)| {
                    let d = |g: &Glyph| {
                        let dy = (p.1 - g.bounds.y - g.bounds.h / 2.0).abs();
                        dy * dy * 10.0 + (p.0 - g.bounds.x).powi(2)
                    };
                    d(a).total_cmp(&d(b))
                })
                .map(|(i, _)| i)
        };
        let (Some(start), Some(end)) = (nearest(a), nearest(b)) else {
            return Default::default();
        };
        let selected = &self.glyphs[start.min(end)..=start.max(end)];
        let text = selected.iter().map(|g| g.ch).collect();
        let mut boxes: Vec<Bounds> = Vec::new();
        for g in selected.iter().filter(|g| g.ch != '\n') {
            if let Some(last) = boxes.last_mut() {
                if (last.y - g.bounds.y).abs() < 2.0 && g.bounds.x <= last.x + last.w + 6.0 {
                    last.w = (g.bounds.x + g.bounds.w - last.x).max(last.w);
                    last.h = last.h.max(g.bounds.h);
                    continue;
                }
            }
            boxes.push(g.bounds);
        }
        (text, boxes)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct Key {
    revision: u64,
    page: i32,
    scale: u32,
    dpi: u32,
    clip: [i32; 4],
    color: [i32; 3],
}
pub struct Cache {
    entries: VecDeque<(Key, Arc<Frame>)>,
    bytes: usize,
    pub budget: usize,
    pub hits: u64,
    pub misses: u64,
}
impl Cache {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
            budget,
            hits: 0,
            misses: 0,
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn render(&mut self, book: &Book, page: i32, scale: f32, dpi: f32) -> Result<Arc<Frame>> {
        self.render_view(book, page, scale, dpi, RenderView::default())
    }
    pub fn render_view(
        &mut self,
        book: &Book,
        page: i32,
        scale: f32,
        dpi: f32,
        view: RenderView,
    ) -> Result<Arc<Frame>> {
        let native = book.engine.load_page(page)?;
        let rect = native.bounds()?;
        ensure!(
            scale.is_finite() && dpi.is_finite() && scale > 0.0 && dpi > 0.0,
            "无效渲染比例"
        );
        let full = IRect::new(
            (rect.x0 * scale).floor() as i32,
            (rect.y0 * scale).floor() as i32,
            (rect.x1 * scale).ceil() as i32,
            (rect.y1 * scale).ceil() as i32,
        );
        let mut clip = full;
        if let Some((w, h, pan)) = view.viewport {
            let w = (w as i32).clamp(1, full.width().max(1));
            let h = (h as i32).clamp(1, full.height().max(1));
            clip.x0 += ((full.width() - w) as f32 * pan[0].clamp(0.0, 1.0)).round() as i32;
            clip.y0 += ((full.height() - h) as f32 * pan[1].clamp(0.0, 1.0)).round() as i32;
            clip.x1 = clip.x0 + w;
            clip.y1 = clip.y0 + h;
        }
        ensure!(
            clip.width() > 0
                && clip.height() > 0
                && clip.width() <= 8192
                && clip.height() <= 8192
                && clip.width() as u64 * clip.height() as u64 <= 20_000_000,
            "页面渲染尺寸超过安全上限"
        );
        let key = Key {
            revision: book.layout_revision,
            page,
            scale: (scale * 1000.0).round() as u32,
            dpi: (dpi * 1000.0).round() as u32,
            clip: [clip.x0, clip.y0, clip.x1, clip.y1],
            color: [
                view.tint.map_or(0, |c| c.0),
                view.tint.map_or(0xffffff, |c| c.1),
                view.invert as i32,
            ],
        };
        if let Some(i) = self.entries.iter().position(|(k, _)| *k == key) {
            let entry = self.entries.remove(i).unwrap();
            let frame = entry.1.clone();
            self.entries.push_back(entry);
            self.hits += 1;
            return Ok(frame);
        }
        self.misses += 1;
        let start = Instant::now();
        let mut pix = native.to_pixmap_region(&Matrix::new_scale(scale, scale), clip)?;
        if view.invert {
            pix.invert()?;
        }
        if let Some((black, white)) = view.tint {
            if (black, white) != (0, 0xffffff) {
                pix.tint(black, white)?;
            }
        }
        ensure!(
            pix.n() == 3 && pix.stride() == (pix.width() * 3) as isize,
            "Unexpected native RGB pixel layout"
        );
        let origin = (pix.x() as f32, pix.y() as f32);
        let bounds = |r: mupdf::Rect| Bounds {
            x: (r.x0 * scale - origin.0) / dpi,
            y: (r.y0 * scale - origin.1) / dpi,
            w: (r.x1 - r.x0) * scale / dpi,
            h: (r.y1 - r.y0) * scale / dpi,
        };
        let structured = native.to_text_page(TextPageFlags::PRESERVE_IMAGES)?;
        let mut glyphs = Vec::new();
        let mut images = Vec::new();
        for block in structured.blocks() {
            match block.r#type() {
                TextBlockType::Text => {
                    for line in block.lines() {
                        for c in line.chars() {
                            let q = c.quad();
                            let r = quad_rect(&q);
                            glyphs.push(Glyph {
                                ch: c.char().unwrap_or('\u{fffd}'),
                                bounds: bounds(r),
                            });
                        }
                        glyphs.push(Glyph {
                            ch: '\n',
                            bounds: Default::default(),
                        });
                    }
                }
                TextBlockType::Image => images.push(bounds(block.bounds())),
                _ => {}
            }
        }
        let links = native
            .links()?
            .map(|l| PageLink {
                bounds: bounds(l.bounds),
                uri: l.uri,
            })
            .collect();
        let frame = Arc::new(Frame {
            pixels: Pixels::new(pix.samples(), pix.width(), pix.height()),
            width: pix.width(),
            height: pix.height(),
            display_width: pix.width() as f32 / dpi,
            display_height: pix.height() as f32 / dpi,
            page,
            glyphs,
            links,
            images,
            scale,
            dpi,
            origin,
            overflow: (
                (full.width() - clip.width()) as f32 / dpi,
                (full.height() - clip.height()) as f32 / dpi,
            ),
            render_ms: start.elapsed().as_secs_f64() * 1000.0,
        });
        let cost = frame.cost();
        while self.bytes + cost > self.budget {
            if let Some((_, old)) = self.entries.pop_front() {
                self.bytes -= old.cost()
            } else {
                break;
            }
        }
        if cost <= self.budget {
            self.bytes += cost;
            self.entries.push_back((key, frame.clone()));
        }
        Ok(frame)
    }
}
