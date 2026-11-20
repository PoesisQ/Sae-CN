use sae_reader::{
    reading::{Navigation, Request, Update, Worker},
    settings::Typography,
};
use std::{sync::mpsc, time::Duration};

#[test]
fn native_pdf_grid_keeps_all_text_links_search_and_bounded_tiles() {
    let (tx, rx) = mpsc::channel();
    let worker = Worker::start(move |g, u| {
        let _ = tx.send((g, u));
    });
    let mut request = Request {
        path: "tests/fixtures/slides.pdf".into(),
        style: Typography::default(),
        width: 1280.0,
        height: 900.0,
        dpi: 1.25,
        page: 0,
        navigation: Navigation::Page(0),
        query: String::new(),
        pdf_zoom: 1.0,
        spread: false,
        view: Default::default(),
    };
    request.view.multi = true;
    let get = |r: Request| -> Update {
        let target = worker.mailbox.submit(r);
        loop {
            let (g, u) = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            if g == target {
                break u.unwrap();
            }
        }
    };
    let first = get(request.clone());
    assert_eq!((first.rows, first.step, first.extra.len()), (2, 4, 2));
    for (i, f) in std::iter::once(&first.frame)
        .chain(first.right.iter())
        .chain(first.extra.iter())
        .enumerate()
    {
        assert_eq!(f.page, i as i32);
        assert!(f.display_height <= 450.5 && f.display_width <= 640.5);
        assert!(!f.links.is_empty());
        let text: String = f.glyphs.iter().map(|g| g.ch).collect();
        assert!(text.contains(&format!("PAGE-{:02}", i + 1)));
        let start = f.glyphs.iter().find(|g| g.ch == 'P').unwrap();
        assert!(
            f.select(
                (start.bounds.x, start.bounds.y),
                (start.bounds.x + 80.0, start.bounds.y)
            )
            .0
            .contains('P')
        );
    }
    assert!(first.cache_bytes <= 16 * 1024 * 1024);
    request.query = "PAGE-04".into();
    request.navigation = Navigation::Search(1);
    let hit = get(request.clone());
    assert_eq!(hit.search_hit.map(|v| v.0), Some(3));
    assert!(!hit.extra_search[1].is_empty());
    request.query.clear();
    request.pdf_zoom = 2.0;
    request.navigation = Navigation::Page(4);
    let zoom = get(request.clone());
    assert_eq!(zoom.frame.page, 4);
    assert!(zoom.extra.iter().all(|f| f.height == 563 && f.width == 800));
    assert!(zoom.cache_bytes <= 16 * 1024 * 1024);
    request.pdf_zoom = 1.0;
    request.navigation = Navigation::Page(8);
    let last = get(request.clone());
    assert_eq!(last.frame.page, 8);
    assert!(last.right.is_none() && last.extra.is_empty());
    request.path = "tests/fixtures/mixed-pages.pdf".into();
    request.navigation = Navigation::Page(4);
    let portrait = get(request.clone());
    assert_eq!((portrait.step, portrait.rows), (4, 2));
    request.path = "tests/fixtures/core.pdf".into();
    request.width = 1706.0;
    request.height = 1066.0;
    request.navigation = Navigation::Page(0);
    let portrait = get(request.clone());
    assert_eq!(
        (portrait.step, portrait.rows, portrait.extra.len()),
        (8, 2, 6)
    );
    request.query = "Paragraph 08-01".into();
    request.navigation = Navigation::Search(1);
    let searched = get(request.clone());
    assert_eq!(searched.search_hit.map(|(p, _)| p), Some(7));
    assert!(!searched.extra_search[5].is_empty());
    request.query.clear();
    for (idx, (cols, rows)) in sae_reader::paging::PDF_GRIDS.into_iter().enumerate() {
        request.view.grid = idx as u8 + 1;
        request.navigation = Navigation::Page(29);
        let end = get(request.clone());
        assert_eq!((end.step, end.rows), (cols * rows, rows));
        assert_eq!(end.frame.page, 29 / end.step * end.step);
        let visible: Vec<_> = std::iter::once(&end.frame)
            .chain(end.right.iter())
            .chain(&end.extra)
            .collect();
        assert_eq!(visible.last().unwrap().page, 29);
        assert!(
            visible
                .iter()
                .all(|f| !f.glyphs.is_empty()
                    && f.display_width <= request.width / cols as f32 + 1.0)
        );
        assert!(end.cache_bytes <= 16 * 1024 * 1024);
    }
    request.width = 640.0;
    let narrow = get(request.clone());
    assert_eq!(narrow.step, 1);
    request.path = "tests/fixtures/core-epub3.epub".into();
    request.navigation = Navigation::Page(0);
    request.width = 1280.0;
    let epub = get(request);
    assert_eq!(epub.step, 1);
    assert!(epub.extra.is_empty());
}
