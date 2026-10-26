//! One replaceable request slot: at most one executing and one pending request.
use crate::{
    document::{Book, ContentsItem},
    rendering::{Bounds, Cache, Frame, RenderView},
    settings::Typography,
};
use anyhow::Result;
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

#[derive(Clone)]
pub enum Navigation {
    Keep,
    Page(i32),
    Link(String),
    Anchor(u64),
    Search(i32),
}
#[derive(Clone)]
pub struct ViewOptions {
    pub encoding: Option<String>,
    pub color: u8,
    pub fit_width: bool,
    pub fit_cover: bool,
    pub pan: [f32; 2],
    pub search_hit: Option<(i32, usize)>,
}
impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            encoding: None,
            color: 0,
            fit_width: false,
            fit_cover: false,
            pan: [0.5, 0.0],
            search_hit: None,
        }
    }
}
#[derive(Clone)]
pub struct Request {
    pub path: PathBuf,
    pub style: Typography,
    pub width: f32,
    pub height: f32,
    pub dpi: f32,
    pub page: i32,
    pub navigation: Navigation,
    pub query: String,
    pub pdf_zoom: f32,
    pub spread: bool,
    pub view: ViewOptions,
}
#[derive(Clone)]
pub struct Update {
    pub frame: Arc<Frame>,
    pub right: Option<Arc<Frame>>,
    pub title: String,
    pub count: i32,
    pub pagination_complete: bool,
    pub reflowable: bool,
    pub contents: Arc<Vec<ContentsItem>>,
    pub anchor: u64,
    pub right_anchor: Option<u64>,
    pub search: Vec<Bounds>,
    pub right_search: Vec<Bounds>,
    pub cache_bytes: usize,
    pub cache_hit: u64,
    pub cache_miss: u64,
    pub notice: String,
    pub elapsed_ms: f64,
    pub search_status: String,
    pub search_hit: Option<(i32, usize)>,
    pub encoding: Option<String>,
    pub pan: [f32; 2],
    pub pdf_fit_width_zoom: f32,
    pub pdf_fill_zoom: f32,
    pub input_pan: [f32; 2],
}
struct Pending {
    request: Request,
    generation: u64,
}
pub struct Mailbox {
    pending: Mutex<Option<Pending>>,
    wake: Condvar,
    generation: AtomicU64,
    stop: AtomicBool,
}
pub struct Worker {
    pub mailbox: Arc<Mailbox>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Mailbox {
    pub fn submit(&self, request: Request) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.pending.lock().unwrap() = Some(Pending {
            request,
            generation,
        });
        self.wake.notify_one();
        generation
    }
    pub fn current(&self, generation: u64) -> bool {
        !self.stop.load(Ordering::Relaxed) && self.generation.load(Ordering::SeqCst) == generation
    }
}
impl Worker {
    pub fn start(notify: impl Fn(u64, Result<Update>) + Send + 'static) -> Self {
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(None),
            wake: Condvar::new(),
            generation: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        });
        let inbox = mailbox.clone();
        let handle = thread::Builder::new()
            .name("document-worker".into())
            .spawn(move || {
                crate::fonts::install();
                let mut book: Option<Book> = None;
                let mut cache = Cache::new(16 * 1024 * 1024);
                let mut opened_encoding = None;
                let mut fixed_view = None;
                let mut actual_page = 0;
                loop {
                    let pending = {
                        let mut slot = inbox.pending.lock().unwrap();
                        while slot.is_none() && !inbox.stop.load(Ordering::Relaxed) {
                            slot = inbox.wake.wait(slot).unwrap();
                        }
                        if inbox.stop.load(Ordering::Relaxed) {
                            break;
                        }
                        slot.take().unwrap()
                    };
                    let g = pending.generation;
                    let request = pending.request;
                    let started = std::time::Instant::now();
                    let result = (|| -> Result<Update> {
                        let mut retained_anchor = None;
                        let conversion_changed = book.as_ref().is_some_and(|b| {
                            b.path == request.path
                                && b.conversion_style.as_ref().is_some_and(|s| {
                                    s != &(
                                        request.style.foreground.clone(),
                                        request.style.family.clone(),
                                    )
                                })
                        });
                        if conversion_changed && matches!(request.navigation, Navigation::Keep) {
                            retained_anchor = book
                                .as_ref()
                                .and_then(|b| b.engine.logical_anchor(actual_page).ok());
                        }
                        if book.as_ref().is_none_or(|b| b.path != request.path)
                            || conversion_changed
                            || opened_encoding != request.view.encoding
                        {
                            book = None;
                            cache.clear();
                            book = Some(Book::open_styled(
                                &request.path,
                                request.view.encoding.as_deref(),
                                &request.style,
                            )?);
                            opened_encoding = request.view.encoding.clone();
                            actual_page = 0;
                        }
                        let b = book.as_mut().unwrap();
                        if !b.reflowable {
                            let colors = if request.view.color == 1 {
                                [
                                    u32::from_str_radix(
                                        request.style.foreground.trim_start_matches('#'),
                                        16,
                                    )
                                    .unwrap_or(0),
                                    u32::from_str_radix(
                                        request.style.background.trim_start_matches('#'),
                                        16,
                                    )
                                    .unwrap_or(0xffffff),
                                ]
                            } else {
                                [0, 0]
                            };
                            let view = [
                                request.width.to_bits(),
                                request.height.to_bits(),
                                request.dpi.to_bits(),
                                request.pdf_zoom.to_bits(),
                                request.spread as u32,
                                request.view.fit_width as u32,
                                request.view.fit_cover as u32,
                                request.view.color as u32,
                                colors[0],
                                colors[1],
                            ];
                            if fixed_view != Some(view) {
                                cache.clear();
                                fixed_view = Some(view);
                            }
                        } else {
                            fixed_view = None;
                        }
                        let spread = request.spread && request.width >= 800.0;
                        let column = if spread {
                            (request.width - 32.0) / 2.0
                        } else {
                            request.width
                        };
                        let width = request.style.width.min(column).max(160.0) * 0.75;
                        let height = request.height.max(160.0) * 0.75;
                        let old_revision = b.layout_revision;
                        actual_page = b.layout(&request.style, width, height, actual_page)?;
                        if old_revision != b.layout_revision {
                            cache.clear();
                        }
                        if !inbox.current(g) {
                            anyhow::bail!("superseded");
                        }
                        actual_page = match &request.navigation {
                            Navigation::Page(p) => *p,
                            Navigation::Link(uri) => b
                                .engine
                                .resolve_link(uri)?
                                .map(|d| d.loc.page_number as i32)
                                .unwrap_or(actual_page),
                            Navigation::Anchor(anchor) => {
                                b.engine.resolve_anchor(*anchor)?.unwrap_or(actual_page)
                            }
                            Navigation::Keep | Navigation::Search(_) => retained_anchor
                                .and_then(|a| b.engine.resolve_anchor(a).ok().flatten())
                                .unwrap_or(actual_page),
                        }
                        .max(0);
                        b.ensure_page(actual_page)?;
                        actual_page = actual_page.min(b.page_count - 1);
                        let previous_hit = if old_revision == b.layout_revision {
                            request.view.search_hit
                        } else {
                            None
                        };
                        let mut search_hit = previous_hit;
                        let mut search_status = String::new();
                        let mut pan = request.view.pan;
                        if let Navigation::Search(direction) = request.navigation {
                            // No persistent index, text corpus or second engine thread. Native
                            // text pages are searched and released one at a time, with cancellation.
                            while !b.pagination_complete {
                                if !inbox.current(g) {
                                    anyhow::bail!("superseded");
                                }
                                b.paginate_next()?;
                            }
                            search_hit = None;
                            if !request.query.is_empty() {
                                let step = if direction < 0 { -1 } else { 1 };
                                let search_start = previous_hit
                                    .map(|(page, _)| page)
                                    .filter(|p| *p >= 0 && *p < b.page_count)
                                    .unwrap_or(actual_page);
                                for offset in 0..=b.page_count {
                                    if !inbox.current(g) {
                                        anyhow::bail!("superseded");
                                    }
                                    let page =
                                        (search_start + offset * step).rem_euclid(b.page_count);
                                    let hits =
                                        b.engine.load_page(page)?.search(&request.query, 4096)?;
                                    let previous =
                                        previous_hit.filter(|(p, _)| *p == page).map(|(_, i)| i);
                                    let index = if offset == 0 {
                                        if step > 0 {
                                            previous.map_or(0, |i| i + 1)
                                        } else {
                                            previous.unwrap_or(hits.len()).saturating_sub(1)
                                        }
                                    } else if step > 0 {
                                        0
                                    } else {
                                        hits.len().saturating_sub(1)
                                    };
                                    let valid = !hits.is_empty()
                                        && index < hits.len()
                                        && !(offset == 0 && step < 0 && previous == Some(0));
                                    if valid {
                                        search_hit = Some((page, index));
                                        actual_page = page;
                                        search_status = format!(
                                            "第 {} 页 · 本页高亮 {}/{}",
                                            page + 1,
                                            index + 1,
                                            hits.len()
                                        );
                                        if hits.len() == 4096 {
                                            search_status.push_str(" · 本页达到 4096 个区域上限");
                                        }
                                        if !b.reflowable {
                                            let bounds = b.engine.load_page(page)?.bounds()?;
                                            let scale =
                                                pdf_scale(bounds, column, request.height, &request);
                                            let q = &hits[index];
                                            let full = [
                                                (bounds.x1 - bounds.x0) * scale / request.dpi,
                                                (bounds.y1 - bounds.y0) * scale / request.dpi,
                                            ];
                                            let center = [
                                                ((q.ul.x + q.lr.x) * 0.5 - bounds.x0) * scale
                                                    / request.dpi,
                                                ((q.ul.y + q.lr.y) * 0.5 - bounds.y0) * scale
                                                    / request.dpi,
                                            ];
                                            pan = [
                                                ((center[0] - column * 0.5)
                                                    / (full[0] - column).max(1.0))
                                                .clamp(0.0, 1.0),
                                                ((center[1] - request.height * 0.5)
                                                    / (full[1] - request.height).max(1.0))
                                                .clamp(0.0, 1.0),
                                            ];
                                        }
                                        break;
                                    }
                                }
                                if search_hit.is_none() {
                                    search_status = "全文未找到匹配；扫描件需要文本层".into();
                                }
                            }
                        }
                        if spread {
                            actual_page = crate::paging::spread_start(actual_page);
                            b.ensure_page(actual_page + 1)?;
                        }
                        let frame = render_page(&mut cache, b, actual_page, column, &request, pan)?;
                        let right = if spread && actual_page + 1 < b.page_count {
                            Some(render_page(
                                &mut cache,
                                b,
                                actual_page + 1,
                                column,
                                &request,
                                pan,
                            )?)
                        } else {
                            None
                        };
                        if !inbox.current(g) {
                            anyhow::bail!("superseded");
                        }
                        let search_page = |frame: &Frame| -> Result<Vec<Bounds>> {
                            let mut search = Vec::new();
                            if !request.query.is_empty() {
                                let page = b.engine.load_page(frame.page)?;
                                for (index, q) in
                                    page.search(&request.query, 4096)?.iter().enumerate()
                                {
                                    if index >= 256 && search_hit != Some((frame.page, index)) {
                                        continue;
                                    }
                                    let q = crate::rendering::quad_rect(q);
                                    search.push(Bounds {
                                        x: (q.x0 * frame.scale - frame.origin.0) / request.dpi,
                                        y: (q.y0 * frame.scale - frame.origin.1) / request.dpi,
                                        w: (q.x1 - q.x0) * frame.scale / request.dpi,
                                        h: (q.y1 - q.y0) * frame.scale / request.dpi,
                                    });
                                }
                            }
                            Ok(search)
                        };
                        let search = search_page(&frame)?;
                        let right_search = if let Some(frame) = &right {
                            search_page(frame)?
                        } else {
                            Vec::new()
                        };
                        let anchor = b.engine.logical_anchor(actual_page)?;
                        let right_anchor = if let Some(frame) = &right {
                            Some(b.engine.logical_anchor(frame.page)?)
                        } else {
                            None
                        };
                        let pdf_fit_width_zoom = if b.reflowable {
                            1.0
                        } else {
                            crate::zoom::width_stop(frame.page_size, column, request.height)
                        };
                        let pdf_fill_zoom = if b.reflowable {
                            1.0
                        } else {
                            crate::zoom::fill_stop(frame.page_size, column, request.height)
                        };
                        Ok(Update {
                            frame,
                            right,
                            title: b.title.clone(),
                            count: b.page_count,
                            pagination_complete: b.pagination_complete,
                            reflowable: b.reflowable,
                            contents: b.contents()?,
                            anchor,
                            right_anchor,
                            search,
                            right_search,
                            cache_bytes: cache.bytes(),
                            cache_hit: cache.hits,
                            cache_miss: cache.misses,
                            notice: b.notice.clone(),
                            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
                            search_status,
                            search_hit,
                            encoding: b.encoding.clone(),
                            pan,
                            pdf_fit_width_zoom,
                            pdf_fill_zoom,
                            input_pan: request.view.pan,
                        })
                    })();
                    if inbox.current(g) {
                        match result {
                            Ok(mut update) => {
                                let visible_bytes = update.frame.cost()
                                    + update.right.as_ref().map_or(0, |f| f.cost());
                                // Publish correct visible pages before all-book pagination.
                                notify(g, Ok(update.clone()));
                                if let Some(b) = book.as_mut() {
                                    while !b.pagination_complete && inbox.current(g) {
                                        if let Err(error) = b.paginate_next() {
                                            update.notice =
                                                format!("部分章节暂时无法分页：{error:#}");
                                            break;
                                        }
                                    }
                                    if inbox.current(g)
                                        && (update.count != b.page_count
                                            || update.pagination_complete != b.pagination_complete)
                                    {
                                        update.count = b.page_count;
                                        update.pagination_complete = b.pagination_complete;
                                        notify(g, Ok(update));
                                    }
                                    // Bound prefetch to the next spread; no idle timers or whole-book bitmaps.
                                    let step = if request.spread && request.width >= 800.0 {
                                        2
                                    } else {
                                        1
                                    };
                                    for page in actual_page + step
                                        ..(actual_page + 2 * step).min(b.page_count)
                                    {
                                        if visible_bytes.saturating_mul(2) > cache.budget {
                                            break;
                                        }
                                        if !inbox.current(g) {
                                            break;
                                        }
                                        // Magnified PDF views render on demand; do not prefetch
                                        // tiles that the next page may never use.
                                        if b.reflowable
                                            || (request.pdf_zoom <= 1.0
                                                && !request.view.fit_width
                                                && !request.view.fit_cover)
                                        {
                                            let column = if step == 2 {
                                                (request.width - 32.0) / 2.0
                                            } else {
                                                request.width
                                            };
                                            let _ = render_page(
                                                &mut cache,
                                                b,
                                                page,
                                                column,
                                                &request,
                                                request.view.pan,
                                            );
                                        }
                                    }
                                }
                            }
                            Err(error) => notify(g, Err(error)),
                        }
                    }
                }
                drop(book);
                cache.clear();
                crate::fonts::clear_thread_cache();
            })
            .expect("Cannot start document worker");
        Self {
            mailbox,
            handle: Some(handle),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.mailbox.stop.store(true, Ordering::Relaxed);
        self.mailbox.generation.fetch_add(1, Ordering::SeqCst);
        self.mailbox.wake.notify_all();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn pdf_scale(bounds: mupdf::Rect, column: f32, height: f32, request: &Request) -> f32 {
    let width = column / (bounds.x1 - bounds.x0).max(1.0);
    let fit = if request.view.fit_cover {
        width.max(height / (bounds.y1 - bounds.y0).max(1.0))
    } else if request.view.fit_width {
        width
    } else {
        width.min(height / (bounds.y1 - bounds.y0).max(1.0))
    };
    (fit * request.dpi * request.pdf_zoom).clamp(0.02, 16.0)
}
fn render_page(
    cache: &mut Cache,
    book: &Book,
    page: i32,
    column: f32,
    request: &Request,
    pan: [f32; 2],
) -> Result<Arc<Frame>> {
    if book.reflowable {
        return cache.render(book, page, request.dpi * (96.0 / 72.0), request.dpi);
    }
    let bounds = book.engine.load_page(page)?.bounds()?;
    let scale = pdf_scale(bounds, column, request.height, request);
    let tint = if request.view.color == 1 {
        let parse = |s: &str, default| {
            i32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(default)
        };
        Some((
            parse(&request.style.foreground, 0),
            parse(&request.style.background, 0xffffff),
        ))
    } else {
        None
    };
    cache.render_view(
        book,
        page,
        scale,
        request.dpi,
        RenderView {
            viewport: Some((
                (column * request.dpi).floor().max(1.0) as u32,
                (request.height * request.dpi).floor().max(1.0) as u32,
                pan,
            )),
            tint,
            invert: request.view.color == 2,
        },
    )
}
