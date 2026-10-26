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
