//! One replaceable request slot: at most one executing and one pending request.
use crate::{
    document::{Book, ContentsItem},
    rendering::{Bounds, Cache, Frame},
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
                let mut cache = Cache::new(24 * 1024 * 1024);
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
                        if book.as_ref().is_none_or(|b| b.path != request.path) {
                            book = None;
                            cache.clear();
                            book = Some(Book::open(&request.path)?);
                            actual_page = 0;
                        }
                        let b = book.as_mut().unwrap();
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
                            Navigation::Keep => actual_page,
                        }
                        .max(0);
                        b.ensure_page(actual_page)?;
                        actual_page = actual_page.min(b.page_count - 1);
                        if spread {
                            actual_page = crate::paging::spread_start(actual_page);
                            b.ensure_page(actual_page + 1)?;
                        }
                        let scale = if b.reflowable {
                            request.dpi * (96.0 / 72.0)
                        } else {
                            let page = b.engine.load_page(actual_page)?;
                            let bounds = page.bounds()?;
                            (column / (bounds.x1 - bounds.x0))
                                .min(request.height / (bounds.y1 - bounds.y0))
                                * request.dpi
                                * request.pdf_zoom
                        }
                        .clamp(0.1, 5.0);
                        let frame = cache.render(b, actual_page, scale, request.dpi)?;
                        let right = if spread && actual_page + 1 < b.page_count {
                            Some(cache.render(b, actual_page + 1, scale, request.dpi)?)
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
                                for q in page.search(&request.query, 256)?.iter() {
                                    search.push(Bounds {
                                        x: (q.ul.x * scale - frame.origin.0) / request.dpi,
                                        y: (q.ul.y * scale - frame.origin.1) / request.dpi,
                                        w: (q.ur.x - q.ul.x) * scale / request.dpi,
                                        h: (q.ll.y - q.ul.y) * scale / request.dpi,
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
                        })
                    })();
                    if inbox.current(g) {
                        match result {
                            Ok(mut update) => {
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
                                        if !inbox.current(g) {
                                            break;
                                        }
                                        if let Ok(native) = b.engine.load_page(page) {
                                            if let Ok(bounds) = native.bounds() {
                                                let scale = if b.reflowable {
                                                    request.dpi * (96.0 / 72.0)
                                                } else {
                                                    let column = if step == 2 {
                                                        (request.width - 32.0) / 2.0
                                                    } else {
                                                        request.width
                                                    };
                                                    (column / (bounds.x1 - bounds.x0)).min(
                                                        request.height / (bounds.y1 - bounds.y0),
                                                    ) * request.dpi
                                                        * request.pdf_zoom
                                                }
                                                .clamp(0.1, 5.0);
                                                let _ = cache.render(b, page, scale, request.dpi);
                                            }
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
