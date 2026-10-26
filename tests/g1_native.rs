//! Native end-to-end worker checks: real MuPDF pages, actual pixels/coordinates,
//! relayout, stale-task suppression, cache budget and released Windows file handles.
use sae_reader::{
    reading::{Navigation, Request, Worker},
    settings::Typography,
};
use std::{path::PathBuf, sync::mpsc, time::Duration};

#[test]
fn real_documents_relayout_cancel_and_release() {
    // The running reader may legitimately have the public fixtures open.
    // Exclusive-handle checks must use files owned by this regression process.
    let fixture_dir = std::env::temp_dir().join(format!("sae-handle-test-{}", std::process::id()));
    std::fs::create_dir_all(&fixture_dir).unwrap();
    for name in ["core.pdf", "core-epub3.epub"] {
        std::fs::copy(
            PathBuf::from("tests/fixtures").join(name),
            fixture_dir.join(name),
        )
        .unwrap();
    }
    let (send, receive) = mpsc::channel();
    let worker = Worker::start(move |generation, result| {
        let _ = send.send((generation, result));
    });
    let mut request = Request {
        path: fixture_dir.join("core-epub3.epub"),
        style: Typography::default(),
        width: 1000.0,
        height: 800.0,
        dpi: 1.25,
        page: 0,
        navigation: Navigation::Page(0),
        query: String::new(),
        pdf_zoom: 1.0,
        view: Default::default(),
        spread: false,
    };
    let receive_generation = |target| {
        loop {
            let (generation, result) = receive
                .recv_timeout(Duration::from_secs(30))
                .expect("native worker stalled");
            if generation == target {
                break result.expect("native operation failed");
            }
        }
    };
    let mut last_anchor = 0;
    let mut saw_image = false;
    for page in 0..20 {
        request.navigation = Navigation::Page(page);
        let update = receive_generation(worker.mailbox.submit(request.clone()));
        assert_eq!(update.frame.page, page);
        assert!(update.frame.glyphs.len() > 10);
        assert!(update.cache_bytes <= 24 * 1024 * 1024);
        assert!(
            update.frame.pixels.iter().any(|&b| b < 100),
            "not a blank placeholder"
        );
        if page == 0 {
            assert!(update.frame.glyphs.iter().any(|g| g.ch == '春'));
            assert!(
                !update.pagination_complete,
                "first image must publish before all-book pagination"
            );
        }
        saw_image |= !update.frame.images.is_empty();
        if let Some(glyph) = update.frame.glyphs.iter().find(|g| g.ch == '春') {
            let b = glyph.bounds;
            let (text, boxes) = update
                .frame
                .select((b.x, b.y + b.h / 2.0), (b.x + b.w * 0.1, b.y + b.h / 2.0));
            assert!(text.contains('春'));
            assert!(!boxes.is_empty());
        }
        last_anchor = update.anchor;
    }
    assert!(
        saw_image,
        "illustration missing from twenty real rendered pages"
    );
    request.spread = true;
    request.navigation = Navigation::Page(17);
    let spread = receive_generation(worker.mailbox.submit(request.clone()));
    assert_eq!(spread.frame.page, 16);
    assert_eq!(spread.right.as_ref().unwrap().page, 17);
    assert!(
        spread.frame.display_width + spread.right.as_ref().unwrap().display_width + 32.0
            <= request.width + 2.0
    );
    request.spread = false;
    request.navigation = Navigation::Anchor(last_anchor);
    request.style.size_pt = 26.0;
    request.navigation = Navigation::Anchor(last_anchor);
    let changed = receive_generation(worker.mailbox.submit(request.clone()));
    assert!(
        changed.frame.page >= 19,
        "layout should preserve later text rather than reset"
    );
    assert_eq!(
        changed.anchor >> 32,
        last_anchor >> 32,
        "logical chapter changed"
    );
    let mut final_generation = 0;
    for i in 0..100 {
        request.style.size_pt = 12.0 + (i % 24) as f32;
        request.navigation = Navigation::Anchor(last_anchor);
        final_generation = worker.mailbox.submit(request.clone());
    }
    let last = receive_generation(final_generation);
    assert!(last.cache_bytes <= 24 * 1024 * 1024);
    request.path = fixture_dir.join("core.pdf");
    request.navigation = Navigation::Page(0);
    for page in 0..20 {
        request.navigation = Navigation::Page(page);
        let update = receive_generation(worker.mailbox.submit(request.clone()));
        assert_eq!(update.frame.page, page);
        assert!(!update.reflowable);
        assert_eq!(update.count, 30);
        assert!(!update.frame.glyphs.is_empty());
    }
    drop(worker);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        for path in [
            fixture_dir.join("core.pdf"),
            fixture_dir.join("core-epub3.epub"),
        ] {
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(path)
                .expect("native file handle remains open after worker shutdown");
        }
    }
    std::fs::remove_dir_all(fixture_dir).unwrap();
}

#[test]
fn calibre_style_inline_svg_cover_keeps_its_image() {
    let (send, receive) = mpsc::channel();
    let worker = Worker::start(move |_, result| {
        let _ = send.send(result);
    });
    worker.mailbox.submit(Request {
        path: PathBuf::from("tests/fixtures/inline-svg-cover.epub"),
        style: Typography::default(),
        width: 1000.0,
        height: 740.0,
        dpi: 1.5,
        page: 0,
        navigation: Navigation::Page(0),
        query: String::new(),
        pdf_zoom: 1.0,
        view: Default::default(),
        spread: false,
    });
    let update = receive
        .recv_timeout(Duration::from_secs(15))
        .unwrap()
        .unwrap();
    assert!(
        !update.frame.images.is_empty(),
        "SVG image resource was dropped"
    );
    assert!(
        update
            .frame
            .pixels
            .chunks_exact(3)
            .filter(|p| p[0] < 180 || p[1] < 180 || p[2] < 180)
            .take(1001)
            .count()
            > 1000
    );
    drop(worker);
}
