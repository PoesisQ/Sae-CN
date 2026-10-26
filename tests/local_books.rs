//! Opt-in, local-only regression. User books are never copied into fixtures.
use sae_reader::{
    reading::{Navigation, Request, Worker},
    settings::Typography,
};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
fn local_kf8_huffdic_keeps_native_pages_toc_text_and_anchor_after_relayout() {
    let Some(path) = std::env::var_os("SAE_READER_TEST_AZW3") else {
        return;
    };
    let path = PathBuf::from(path);
    let before = blake3::hash(&std::fs::read(&path).unwrap());
    let start = Instant::now();
    let mut book = sae_reader::document::Book::open(&path).unwrap();
    let mut style = Typography::default();
    let mut cache = sae_reader::rendering::Cache::new(16 * 1024 * 1024);
    for (index, width) in [780.0, 390.0].into_iter().enumerate() {
        book.layout(&style, width, 600.0, 0).unwrap();
        book.ensure_page(19).unwrap();
        let mut glyphs = 0;
        for page in 0..book.page_count.min(20) {
            let frame = cache.render(&book, page, 1.25, 1.25).unwrap();
            glyphs += frame.glyphs.len();
            assert!(cache.bytes() <= 16 * 1024 * 1024);
        }
        assert!(
            glyphs > 100,
            "Kindle book has no rendered text in its first 20 pages"
        );
        let target = 12.min(book.page_count - 1);
        let anchor = book.engine.logical_anchor(target).unwrap();
        let chapter = book.engine.chapter_for_page(target).unwrap();
        style.size_pt += 2.0;
        book.layout(&style, width, 600.0, target).unwrap();
        let restored = book
            .engine
            .resolve_anchor(anchor)
            .unwrap()
            .expect("Kindle anchor lost on relayout");
        assert_eq!(book.engine.chapter_for_page(restored).unwrap(), chapter);
        let contents = book.contents().unwrap();
        assert!(!contents.is_empty(), "Kindle NCX was lost");
        assert!(
            book.engine
                .resolve_link(&contents[contents.len() / 2].uri)
                .unwrap()
                .is_some()
        );
        println!(
            "local-kindle layout={index} first20_pages_and_toc=true elapsed_ms={:.2}",
            start.elapsed().as_secs_f64() * 1000.0
        );
        cache.clear();
    }
    assert_eq!(blake3::hash(&std::fs::read(&path).unwrap()), before);
}

#[test]
fn local_epubs_show_real_pixels_in_single_and_double_pages() {
    let Some(folder) = std::env::var_os("SAE_READER_TEST_BOOKS") else {
        return;
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("epub"))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no EPUB test inputs");
    let (send, receive) = mpsc::channel();
    let worker = Worker::start(move |g, r| {
        let _ = send.send((g, r));
    });
    for (index, path) in paths.into_iter().enumerate() {
        for spread in [false, true] {
            let request = Request {
                path: path.clone(),
                style: Typography::default(),
                width: 1100.0,
                height: 740.0,
                dpi: 1.25,
                page: 0,
                navigation: Navigation::Page(0),
                query: String::new(),
                pdf_zoom: 1.0,
                view: Default::default(),
                spread,
            };
            let start = Instant::now();
            let target = worker.mailbox.submit(request);
            let update = loop {
                let (g, r) = receive
                    .recv_timeout(Duration::from_secs(15))
                    .expect("real-book first frame exceeded 15 seconds");
                if g == target {
                    break r.unwrap();
                }
            };
            assert!(update.frame.width > 300 && update.frame.height > 500);
            assert!(
                update
                    .frame
                    .pixels
                    .chunks_exact(3)
                    .filter(|p| p.iter().any(|v| *v < 180))
                    .take(1001)
                    .count()
                    > 1000,
                "cover content missing"
            );
            if spread {
                assert_eq!(update.right.as_ref().unwrap().page, 1);
            }
            assert!(update.cache_bytes <= 24 * 1024 * 1024);
            println!(
                "sample={} spread={} first_frame_ms={:.2} worker_ms={:.2} pixels={}x{} cache_bytes={}",
                index + 1,
                spread,
                start.elapsed().as_secs_f64() * 1000.0,
                update.elapsed_ms,
                update.frame.width,
                update.frame.height,
                update.cache_bytes
            );
        }
    }
    drop(worker);
}

#[test]
fn local_pdfs_magnify_real_pages_without_allocating_a_full_zoomed_bitmap() {
    let Some(folder) = std::env::var_os("SAE_READER_TEST_BOOKS") else {
        return;
    };
    let mut paths = std::fs::read_dir(folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());
    let (tx, rx) = mpsc::channel();
    let worker = Worker::start(move |g, r| {
        let _ = tx.send((g, r));
    });
    for (index, path) in paths.into_iter().enumerate() {
        for spread in [false, true] {
            let request = Request {
                path: path.clone(),
                style: Typography::default(),
                width: 1100.0,
                height: 740.0,
                dpi: 1.25,
                page: 0,
                navigation: Navigation::Page(0),
                query: String::new(),
                pdf_zoom: 3.0,
                spread,
                view: Default::default(),
            };
            let start = Instant::now();
            let target = worker.mailbox.submit(request);
            let update = loop {
                let (g, r) = rx.recv_timeout(Duration::from_secs(20)).unwrap();
                if g == target {
                    break r.unwrap();
                }
            };
            assert!(update.frame.width <= 1375 && update.frame.height <= 925);
            assert!(update.frame.overflow.0 > 1.0 || update.frame.overflow.1 > 1.0);
            assert!(update.cache_bytes <= 16 * 1024 * 1024);
            assert!(
                update
                    .frame
                    .pixels
                    .chunks_exact(3)
                    .any(|p| p.iter().any(|v| *v < 180))
            );
            println!(
                "pdf-sample={} spread={} zoom=3 first_tile_ms={:.2} pixels={}x{} cache_bytes={}",
                index + 1,
                spread,
                start.elapsed().as_secs_f64() * 1000.0,
                update.frame.width,
                update.frame.height,
                update.cache_bytes
            );
        }
    }
}
