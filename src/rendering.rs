use crate::document::Book;
use anyhow::{Result, ensure};
use mupdf::{Colorspace, Matrix, TextBlockContent, TextPageFlags};
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
pub struct Frame {
    pub pixels: Vec<u8>,
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
    pub render_ms: f64,
}
impl Frame {
    pub fn cost(&self) -> usize {
        self.pixels.len()
            + self.glyphs.len() * std::mem::size_of::<Glyph>()
            + self
                .links
                .iter()
                .map(|l| l.uri.len() + std::mem::size_of::<PageLink>())
                .sum::<usize>()
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
        let key = Key {
            revision: book.layout_revision,
            page,
            scale: (scale * 1000.0).round() as u32,
            dpi: (dpi * 1000.0).round() as u32,
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
        let native = book.engine.load_page(page)?;
        let rect = native.bounds()?;
        ensure!(
            (rect.x1 - rect.x0) * scale < 8192.0 && (rect.y1 - rect.y0) * scale < 8192.0,
            "页面渲染尺寸超过安全上限"
        );
        let pix = native.to_pixmap(
            &Matrix::new_scale(scale, scale),
            &Colorspace::device_rgb(),
            false,
            true,
        )?;
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
        let structured = native
            .to_text_page(TextPageFlags::PRESERVE_IMAGES)?
            .structured();
        let mut glyphs = Vec::new();
        let mut images = Vec::new();
        for block in structured.blocks {
            match block.content {
                TextBlockContent::Text { lines } => {
                    for line in lines {
                        for c in line.chars {
                            let q = c.quad;
                            let r = mupdf::Rect::new(
                                q.ul.x.min(q.ll.x),
                                q.ul.y.min(q.ur.y),
                                q.ur.x.max(q.lr.x),
                                q.ll.y.max(q.lr.y),
                            );
                            glyphs.push(Glyph {
                                ch: c.ch,
                                bounds: bounds(r),
                            });
                        }
                        glyphs.push(Glyph {
                            ch: '\n',
                            bounds: Default::default(),
                        });
                    }
                }
                TextBlockContent::Image { .. } => images.push(bounds(block.bounds)),
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
            pixels: pix.samples().to_vec(),
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
