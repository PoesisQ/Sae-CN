//! Real native regression for on-demand adapters, whole-book search and PDF tiles.
use sae_reader::{
    document::Book,
    reading::{Navigation, Request, Worker},
    settings::Typography,
};
use std::{path::PathBuf, sync::mpsc, time::Duration};
fn request(file: &str) -> Request {
    Request {
        path: PathBuf::from(file),
        style: Typography::default(),
        width: 1100.0,
        height: 720.0,
        dpi: 1.25,
        page: 0,
        navigation: Navigation::Page(0),
        query: String::new(),
        pdf_zoom: 1.0,
        spread: false,
        view: Default::default(),
    }
}

#[test]
fn concurrent_worker_contexts_keep_their_own_css_storage() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|id| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut context = mupdf::Context::get();
                let css = format!("body {{ font-size: {}pt; }}", id + 12);
                barrier.wait();
                for _ in 0..500 {
                    context.set_user_css(&css).unwrap();
                    std::thread::yield_now();
                    assert_eq!(context.user_css().as_deref(), Some(css.as_str()));
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn whole_book_search_reaches_other_chapters_and_wraps_in_both_directions() {
    let (tx, rx) = mpsc::channel();
    let worker = Worker::start(move |g, r| {
        let _ = tx.send((g, r));
    });
    let get = |r: Request| {
        let target = worker.mailbox.submit(r);
        loop {
            let (g, r) = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            if g == target {
                break r.unwrap();
            }
        }
    };
    let mut r = request("tests/fixtures/core-epub3.epub");
    let first = get(r.clone());
    assert_eq!(first.frame.page, 0);
    r.query = "Paragraph 30-17".into();
    r.navigation = Navigation::Search(1);
    let found = get(r.clone());
    let (page, index) = found.search_hit.expect("cross-chapter search failed");
    assert!(page > 100 && !found.search.is_empty());
    r.view.search_hit = Some((page, index));
    r.navigation = Navigation::Search(-1);
    let wrapped = get(r.clone());
    assert_eq!(wrapped.search_hit, Some((page, index)));
    r.query = "SAE-NO-SUCH-TEXT-987".into();
    r.view.search_hit = None;
    r.navigation = Navigation::Search(1);
    let missing = get(r.clone());
    assert!(missing.search_hit.is_none() && missing.search_status.contains("未找到"));
    r.spread = true;
    r.query = "Paragraph 10-17".into();
    r.navigation = Navigation::Search(1);
    let spread = get(r);
    let (page, _) = spread.search_hit.unwrap();
    assert!(page == spread.frame.page || Some(page) == spread.right.as_ref().map(|f| f.page));
    assert!(!spread.search.is_empty() || !spread.right_search.is_empty());
    let old_hit = spread.search_hit;
    let hit_count = if page == spread.frame.page {
        spread.search.len()
    } else {
        spread.right_search.len()
    };
    let mut next = request("tests/fixtures/core-epub3.epub");
    next.spread = true;
    next.query = "Paragraph 10-17".into();
    next.view.search_hit = old_hit;
    next.navigation = Navigation::Search(1);
    let wrapped = get(next);
    assert_eq!(
        wrapped.search_hit,
        Some((page, (old_hit.unwrap().1 + 1) % hit_count)),
        "spread cursor should resume on the actual hit page"
    );
}

#[test]
fn pdf_zoom_is_bounded_by_viewport_in_single_and_double_page_and_native_colors_are_cached() {
    let (tx, rx) = mpsc::channel();
    let worker = Worker::start(move |g, r| {
        let _ = tx.send((g, r));
    });
    let get = |r: Request| {
        let target = worker.mailbox.submit(r);
        loop {
            let (g, r) = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            if g == target {
                break r.unwrap();
            }
        }
    };
    let mut r = request("tests/fixtures/core.pdf");
    let plain = get(r.clone());
    assert_eq!(&plain.frame.pixels[0..3], &[255, 255, 255]);
    r.view.color = 1;
    let warm = get(r.clone());
    assert_eq!(&warm.frame.pixels[0..3], &[248, 246, 241]);
    r.view.color = 2;
    let inverse = get(r.clone());
    assert_eq!(&inverse.frame.pixels[0..3], &[0, 0, 0]);
    let unchanged = get(r.clone());
    assert!(std::sync::Arc::ptr_eq(&inverse.frame, &unchanged.frame));
    r.view.color = 0;
    for spread in [false, true] {
        r.spread = spread;
        r.pdf_zoom = 8.0;
        r.view.pan = [0.0, 0.0];
        let top = get(r.clone());
        assert!(top.frame.overflow.0 > 100.0 && top.frame.overflow.1 > 100.0);
        assert!(top.frame.width <= (if spread { 668 } else { 1375 }) && top.frame.height <= 900);
        assert!(top.cache_bytes <= 16 * 1024 * 1024);
        if spread {
            assert!(top.right.as_ref().unwrap().width <= 668);
        }
        r.view.pan = [1.0, 1.0];
        let bottom = get(r.clone());
        let tile_budget = bottom.frame.cost() + bottom.right.as_ref().map_or(0, |f| f.cost());
        assert!(
            bottom.cache_bytes <= tile_budget,
            "obsolete pan tiles retained"
        );
        assert!(
            bottom.frame.origin.0 > top.frame.origin.0
                && bottom.frame.origin.1 > top.frame.origin.1
        );
        assert!(
            bottom
                .frame
                .glyphs
                .iter()
                .all(|g| g.bounds.x.is_finite() && g.bounds.y.is_finite())
        );
    }
    r.spread = false;
    r.query = "Paragraph 01-24".into();
    r.navigation = Navigation::Search(1);
    let found = get(r);
    assert!(found.search_hit.is_some());
    assert!(found.search.iter().any(|b| b.x >= -2.0
        && b.y >= -2.0
        && b.x < found.frame.display_width
        && b.y < found.frame.display_height));
}

#[test]
fn decoded_txt_and_markdown_preserve_real_text_images_tables_links_and_original_bytes() {
    for name in [
        "text-utf8.txt",
        "text-utf16le.txt",
        "text-gb18030.txt",
        "reading.md",
    ] {
        let path = PathBuf::from("tests/fixtures").join(name);
        let before = std::fs::read(&path).unwrap();
        let mut book = Book::open(&path).unwrap();
        let style = Typography::default();
        book.layout(&style, 540.0, 600.0, 0).unwrap();
        while !book.pagination_complete {
            book.paginate_next().unwrap();
        }
        let mut text = String::new();
        let (mut images, mut links) = (0, 0);
        for page in 0..book.page_count {
            let native = book.engine.load_page(page).unwrap();
            text.push_str(&native.text(Default::default()).unwrap());
            images += native
                .to_text_page(mupdf::TextPageFlags::PRESERVE_IMAGES)
                .unwrap()
                .blocks()
                .filter(|b| b.r#type() == mupdf::text_page::TextBlockType::Image)
                .count();
            links += native.links().unwrap().count();
        }
        assert!(text.contains("编码验证"), "{name}: decoding lost CJK");
        if name.ends_with(".md") {
            assert!(text.contains("TABLE-CELL") && text.contains("fn main"));
            assert!(images > 0 && links > 0);
            assert!(book.contents().unwrap().len() >= 2);
        }
        drop(book);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn advanced_conversion_keeps_svg_resources_and_fixed_canvas_is_independent_of_reader_typography() {
    let mut book = Book::open(&PathBuf::from("tests/fixtures/advanced.epub")).unwrap();
    book.layout(&Typography::default(), 540.0, 600.0, 0)
        .unwrap();
    assert!(book.notice.contains("Ruby 1") && book.notice.contains("MathML 1"));
    let page = book.engine.load_page(0).unwrap();
    let images = page
        .to_text_page(mupdf::TextPageFlags::PRESERVE_IMAGES)
        .unwrap();
    assert!(
        images
            .blocks()
            .filter(|b| b.r#type() == mupdf::text_page::TextBlockType::Image)
            .count()
            >= 1
    );
    let text = page.text(Default::default()).unwrap();
    assert!(
        text.contains('漢') && text.contains('字') && text.contains("かん"),
        "SVG text layer was lost: {text}"
    );
    assert!(
        !page.search("漢字", 32).unwrap().is_empty(),
        "Ruby base text is not searchable"
    );
    let mut fixed = Book::open(&PathBuf::from("tests/fixtures/fixed.epub")).unwrap();
    fixed
        .layout(&Typography::default(), 300.0, 300.0, 0)
        .unwrap();
    assert!(!fixed.reflowable);
    assert_eq!(fixed.page_count, 1);
    let page = fixed.engine.load_page(0).unwrap();
    let bounds = page.bounds().unwrap();
    assert!((bounds.x1 - 600.0).abs() < 1.0 && (bounds.y1 - 800.0).abs() < 1.0);
    assert!(
        page.text(Default::default())
            .unwrap()
            .contains("bottom caption")
    );
    let images = page
        .to_text_page(mupdf::TextPageFlags::PRESERVE_IMAGES)
        .unwrap();
    let image = images
        .blocks()
        .find(|b| b.r#type() == mupdf::text_page::TextBlockType::Image)
        .unwrap()
        .bounds();
    assert!(
        (image.x0 - 100.0).abs() < 1.0 && (image.y0 - 250.0).abs() < 1.0,
        "fixed illustration position changed: {image:?}"
    );
    let mut style = Typography::default();
    style.size_pt = 36.0;
    fixed.layout(&style, 800.0, 800.0, 0).unwrap();
    assert_eq!(fixed.layout_revision, 1);
}

#[test]
fn mapped_font_outlives_its_original_rust_handle() {
    let path = std::fs::canonicalize("tests/fixtures/fixture-font.ttf").unwrap();
    let font = mupdf::Font::from_file_mapped("fixture", 0, path.to_str().unwrap()).unwrap();
    let clone = font.clone();
    drop(font);
    assert_ne!(clone.encode_character('E' as i32).unwrap(), 0);
    assert!(
        clone
            .advance_glyph(clone.encode_character('E' as i32).unwrap())
            .unwrap()
            > 0.0
    );
}

#[test]
fn text_typography_controls_change_native_layout_instead_of_being_blocked_by_adapter_css() {
    let mut book = Book::open(&PathBuf::from("tests/fixtures/text-utf8.txt")).unwrap();
    let style = Typography::default();
    book.layout(&style, 540.0, 600.0, 0).unwrap();
    while !book.pagination_complete {
        book.paginate_next().unwrap();
    }
    let before = book.page_count;
    let mut changed = style.clone();
    changed.paragraph_after = 1.8;
    book.layout(&changed, 540.0, 600.0, 0).unwrap();
    while !book.pagination_complete {
        book.paginate_next().unwrap();
    }
    assert!(
        book.page_count > before,
        "paragraph spacing control was overridden by adapter CSS"
    );
    changed.size_pt = 28.0;
    changed.indent = 0.0;
    book.layout(&changed, 420.0, 600.0, 0).unwrap();
    let page = book.engine.load_page(0).unwrap();
    let structured = page.to_text_page(mupdf::TextPageFlags::empty()).unwrap();
    let sizes = structured
        .blocks()
        .flat_map(|b| {
            b.lines()
                .flat_map(|l| l.chars().map(|c| c.size()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(sizes.iter().any(|s| (*s - 28.0).abs() < 0.5));
}
