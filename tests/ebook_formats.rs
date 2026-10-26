//! Real parsing/rendering, including original image/table/link fixtures.
use sae_reader::{document::Book, rendering::Cache, settings::Typography};
use std::path::{Path, PathBuf};

fn read_book(path: &Path) -> (String, usize, usize, usize) {
    let hash = blake3::hash(&std::fs::read(path).unwrap());
    let mut book = Book::open(path).unwrap_or_else(|e| panic!("format open failed: {e:#}"));
    assert!(book.reflowable);
    let style = Typography::default();
    book.layout(&style, 540.0, 600.0, 0).unwrap();
    while !book.pagination_complete {
        book.paginate_next().unwrap();
    }
    let mut text = String::new();
    let (mut images, mut links) = (0, 0);
    let mut cache = Cache::new(4 * 1024 * 1024);
    for page in 0..book.page_count {
        let frame = cache.render(&book, page, 1.0, 1.0).unwrap();
        text.extend(frame.glyphs.iter().map(|g| g.ch));
        images += frame.images.len();
        links += frame.links.len();
        assert!(cache.bytes() <= 4 * 1024 * 1024);
    }
    let contents = book.contents().unwrap().len();
    assert_eq!(blake3::hash(&std::fs::read(path).unwrap()), hash);
    (text, images, links, contents)
}

#[test]
fn kindle_uncompressed_palmdoc_and_kf8_keep_cjk_images_tables_and_links() {
    for name in [
        "kindle-plain.mobi",
        "kindle-palmdoc.mobi",
        "kindle-minimal.azw3",
    ] {
        let (text, images, links, _) = read_book(&PathBuf::from("tests/fixtures").join(name));
        for expected in [
            "SEARCH-KINDLE",
            "中文排版",
            "Table A",
            "Table B",
            "Paragraph 089",
        ] {
            assert!(
                text.chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>()
                    .contains(
                        &expected
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect::<String>()
                    ),
                "{name} lost expected text: {expected}"
            );
        }
        assert!(images > 0, "{name} lost its image");
        assert!(links > 0, "{name} lost its link");
    }
}

#[test]
fn new_formats_search_beyond_the_first_screen_and_keep_spread_hit_coordinates() {
    use sae_reader::reading::{Navigation, Request, Worker};
    use std::{sync::mpsc, time::Duration};
    let (tx, rx) = mpsc::channel();
    let worker = Worker::start(move |generation, result| {
        let _ = tx.send((generation, result));
    });
    let get = |request: Request| {
        let expected = worker.mailbox.submit(request);
        loop {
            let (generation, result) = rx.recv_timeout(Duration::from_secs(20)).unwrap();
            if generation == expected {
                break result.unwrap();
            }
        }
    };
    for (name, query) in [
        ("kindle-plain.mobi", "Paragraph 089"),
        ("kindle-palmdoc.mobi", "Paragraph 089"),
        ("kindle-minimal.azw3", "Paragraph 089"),
        ("reading.fb2", "SECOND-CHAPTER"),
    ] {
        for spread in [false, true] {
            let mut request = Request {
                path: PathBuf::from("tests/fixtures").join(name),
                style: Typography::default(),
                width: 1100.0,
                height: 720.0,
                dpi: 1.25,
                page: 0,
                navigation: Navigation::Page(0),
                query: String::new(),
                pdf_zoom: 1.0,
                spread,
                view: Default::default(),
            };
            assert_eq!(get(request.clone()).frame.page, 0);
            request.query = query.into();
            request.navigation = Navigation::Search(1);
            let found = get(request.clone());
            let (page, _) = found
                .search_hit
                .expect("new format whole-book search failed");
            assert!(page > 0, "search did not leave the first screen");
            assert!(
                page == found.frame.page || found.right.as_ref().is_some_and(|f| f.page == page)
            );
            assert!(!found.search.is_empty() || !found.right_search.is_empty());
            assert!(found.cache_bytes <= 16 * 1024 * 1024);
            request.view.search_hit = found.search_hit;
            request.navigation = Navigation::Search(-1);
            assert_eq!(
                get(request).search_hit,
                found.search_hit,
                "single target did not wrap"
            );
        }
    }
}

#[test]
fn epub3_nav_only_retains_cjk_english_labels_and_nested_outline() {
    let mut book = Book::open(Path::new("tests/fixtures/catalog.epub")).unwrap();
    book.layout(&Typography::default(), 540.0, 600.0, 0)
        .unwrap();
    let contents = book.contents().unwrap();
    assert!(contents[0].title.contains("序章"));
    assert_eq!(contents[1].depth, 1);
    assert!(contents.iter().any(|c| c.title == "Morning letters"));
    assert!(
        contents.len() >= 30,
        "long virtualized TOC fixture lost rows"
    );
    assert!(contents.iter().any(|c| c.title.contains("滚动条")));
    assert!(
        book.engine
            .resolve_link(&contents[1].uri)
            .unwrap()
            .is_some()
    );
}

#[test]
fn fb2_keeps_cjk_outline_base64_image_table_note_and_backlink() {
    let (text, images, links, contents) = read_book(Path::new("tests/fixtures/reading.fb2"));
    for expected in [
        "SEARCH-FB2",
        "中文段落",
        "Table A",
        "Table B",
        "FOOTNOTE-FB2",
        "SECOND-CHAPTER",
    ] {
        assert!(
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .contains(
                    &expected
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>()
                ),
            "FB2 lost expected text: {expected}"
        );
    }
    assert!(images > 0 && links >= 3);
    assert!(contents >= 2, "FB2 chapter titles missing from outline");
}

#[test]
fn fb2_themes_cover_page_margins_and_leave_original_picture_colors_intact() {
    let mut book = Book::open(Path::new("tests/fixtures/reading.fb2")).unwrap();
    let mut style = Typography::default();
    let mut cache = Cache::new(4 * 1024 * 1024);
    for (background, foreground, expected) in [
        ("#F8F6F1", "#262A27", [248, 246, 241]),
        ("#171A19", "#E7E9E5", [23, 26, 25]),
        ("#FFFFFF", "#262A27", [255, 255, 255]),
    ] {
        style.background = background.into();
        style.foreground = foreground.into();
        book.layout(&style, 540.0, 600.0, 0).unwrap();
        cache.clear();
        let frame = cache.render(&book, 0, 1.0, 1.0).unwrap();
        for (x, y) in [
            (0, 0),
            (frame.width - 1, 0),
            (0, frame.height - 1),
            (frame.width - 1, frame.height - 1),
        ] {
            let at = ((y * frame.width + x) * 3) as usize;
            assert_eq!(
                &frame.pixels[at..at + 3],
                &expected,
                "FB2 page edge ignored theme"
            );
        }
        assert!(
            frame
                .pixels
                .chunks_exact(3)
                .filter(|p| *p == [80, 108, 96])
                .count()
                > 100,
            "FB2 theme changed original illustration colors"
        );
        assert!(!frame.images.is_empty() && !frame.glyphs.is_empty());
    }
}

#[test]
fn fb2_without_ids_gets_working_toc_and_legacy_encoding_is_decoded_strictly() {
    let original = std::fs::read_to_string("tests/fixtures/reading.fb2").unwrap();
    let path = std::env::temp_dir().join(format!("sae-fb2-navigation-{}.fb2", std::process::id()));
    let no_ids = original.replace(" id=\"c1\"", "").replace(" id=\"c2\"", "");
    std::fs::write(&path, &no_ids).unwrap();
    {
        let mut book = Book::open(&path).unwrap();
        book.layout(&Typography::default(), 540.0, 600.0, 0)
            .unwrap();
        for row in book.contents().unwrap().iter() {
            assert!(
                book.engine.resolve_link(&row.uri).unwrap().is_some(),
                "FB2 generated TOC target did not resolve"
            );
        }
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), no_ids);
    std::fs::write(&path,"<FictionBook><body><section><title><p>Encoding = plain UTF-8</p></title><p>中文 SEARCH-FB2</p></section></body></FictionBook>").unwrap();
    let (text, _, _, _) = read_book(&path);
    assert!(
        text.contains("SEARCH-FB2"),
        "body text was mistaken for an encoding declaration"
    );
    let russian = "<?xml version=\"1.0\" encoding=\"windows-1251\"?><FictionBook><body><section><title><p>Глава</p></title><p>Привет мир SEARCH-FB2</p></section></body></FictionBook>";
    let (bytes, _, errors) = encoding_rs::WINDOWS_1251.encode(russian);
    assert!(!errors);
    std::fs::write(&path, &bytes).unwrap();
    let (text, _, _, _) = read_book(&path);
    assert!(text.contains("Привет"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn fb2_css_changes_reparse_real_glyphs_and_keep_durable_flow_anchors() {
    let path = Path::new("tests/fixtures/reading.fb2");
    let mut book = Book::open(path).unwrap();
    let mut style = Typography::default();
    book.layout(&style, 540.0, 600.0, 0).unwrap();
    book.ensure_page(5).unwrap();
    let anchor = book.engine.logical_anchor(4).unwrap();
    assert!(
        anchor >> 32 != 0,
        "FB2 stored an absolute page instead of a source anchor"
    );
    let mut cache = Cache::new(4 * 1024 * 1024);
    let before = cache.render(&book, 4, 1.0, 1.0).unwrap();
    let height = before
        .glyphs
        .iter()
        .map(|g| g.bounds.h)
        .fold(0.0f32, f32::max);
    style.size_pt = 28.0;
    style.line_height = 1.9;
    style.margins[1] = 75.0;
    book.layout(&style, 540.0, 600.0, 4).unwrap();
    let page = book.engine.resolve_anchor(anchor).unwrap().unwrap();
    book.ensure_page(page).unwrap();
    let after = cache.render(&book, page, 1.0, 1.0).unwrap();
    assert!(
        after
            .glyphs
            .iter()
            .map(|g| g.bounds.h)
            .fold(0.0f32, f32::max)
            > height * 1.25
    );
    drop(book);
    let mut reopened = Book::open_styled(path, None, &style).unwrap();
    reopened.layout(&style, 540.0, 600.0, 0).unwrap();
    assert_eq!(reopened.engine.resolve_anchor(anchor).unwrap(), Some(page));
    let mut initial_custom = Book::open(path).unwrap();
    initial_custom.layout(&style, 540.0, 600.0, 0).unwrap();
    cache.clear();
    let actual = cache.render(&initial_custom, 0, 1.0, 1.0).unwrap();
    cache.clear();
    let expected = cache.render(&reopened, 0, 1.0, 1.0).unwrap();
    assert_eq!(actual.width, expected.width);
    assert_eq!(
        &actual.pixels[..],
        &expected.pixels[..],
        "initial custom FB2 CSS did not really reparse"
    );
}

#[test]
fn encrypted_truncated_and_excessive_kindle_records_fail_then_normal_open_recovers() {
    let input = std::fs::read("tests/fixtures/kindle-plain.mobi").unwrap();
    let tmp =
        std::env::temp_dir().join(format!("sae-format-regression-{}.azw3", std::process::id()));
    let offset = u32::from_be_bytes(input[78..82].try_into().unwrap()) as usize;
    let mut drm = input.clone();
    drm[offset + 12..offset + 14].copy_from_slice(&2u16.to_be_bytes());
    std::fs::write(&tmp, drm).unwrap();
    let error = Book::open(&tmp).err().expect("DRM was accepted");
    assert!(format!("{error:#}").contains("DRM"));
    for n in [0, 77, offset + 5, input.len() - 1] {
        std::fs::write(&tmp, &input[..n]).unwrap();
        // The final cut can leave a parseable PNG resource; shorter header/table
        // cuts must be rejected. Loading a damaged resource never changes input.
        if n < offset + 20 {
            assert!(Book::open(&tmp).is_err());
        }
    }
    let mut too_large = input.clone();
    too_large[offset + 8..offset + 10].copy_from_slice(&u16::MAX.to_be_bytes());
    for _ in 0..5 {
        std::fs::write(&tmp, &too_large).unwrap();
        assert!(Book::open(&tmp).is_err());
        assert!(Book::open(Path::new("tests/fixtures/kindle-plain.mobi")).is_ok());
    }
    std::fs::remove_file(tmp).unwrap();
}

#[test]
fn opt_in_upstream_huffdic_full_kf8_fonts_and_hybrid_regression() {
    let Some(folder) = std::env::var_os("SAE_READER_UPSTREAM_SAMPLES") else {
        return;
    };
    for name in [
        "sample-unicode-huffdic.mobi",
        "sample-obfuscated-fonts.mobi",
        "sample-ncx.mobi",
    ] {
        let path = PathBuf::from(&folder).join(name);
        let (text, _, _, contents) = read_book(&path);
        let minimum = if name == "sample-ncx.mobi" { 20 } else { 100 };
        assert!(text.chars().filter(|c| c.is_alphanumeric()).count() > minimum);
        if name == "sample-ncx.mobi" {
            assert!(contents > 0);
        }
        println!("upstream format case={name} native read/render passed");
    }
}
