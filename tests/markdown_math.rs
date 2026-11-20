//! Native engine tests: no Slint event loop, desktop window or user document.
use sae_reader::{document::Book, settings::Typography};
use std::path::Path;

#[test]
fn markdown_math_is_vector_text_with_structural_layout_and_code_is_untouched() {
    let path = Path::new("tests/fixtures/math.md");
    let original = std::fs::read(path).unwrap();
    let mut book = Book::open(path).unwrap();
    assert_eq!(book.notice, "1 处公式保留原文。");
    book.layout(&Typography::default(), 540.0, 700.0, 0)
        .unwrap();
    while !book.pagination_complete {
        book.paginate_next().unwrap();
    }
    let mut text = String::new();
    let mut links = 0;
    let mut math_glyphs = Vec::new();
    let mut all_glyphs = Vec::new();
    for number in 0..book.page_count {
        let page = book.engine.load_page(number).unwrap();
        if let Some(folder) = std::env::var_os("SAE_MATH_EVIDENCE_DIR") {
            std::fs::create_dir_all(&folder).unwrap();
            page.to_pixmap(
                &mupdf::Matrix::new_scale(1.5, 1.5),
                &mupdf::Colorspace::device_rgb(),
                false,
                true,
            )
            .unwrap()
            .save_as(
                Path::new(&folder)
                    .join(format!("warm-{number}.png"))
                    .to_str()
                    .unwrap(),
                mupdf::ImageFormat::PNG,
            )
            .unwrap();
        }
        text.push_str(&page.text(Default::default()).unwrap());
        links += page.links().unwrap().count();
        let native = page.to_text_page(mupdf::TextPageFlags::empty()).unwrap();
        for block in native.blocks() {
            for line in block.lines() {
                for ch in line.chars() {
                    if let Some(c) = ch.char() {
                        all_glyphs.push((c, ch.origin()));
                    }
                    if ch
                        .font()
                        .is_some_and(|font| font.name().to_lowercase().contains("cambria"))
                    {
                        math_glyphs.push((ch.char(), ch.origin(), ch.size()));
                    }
                }
            }
        }
    }
    assert!(
        math_glyphs.len() > 60,
        "formula text layer lost; {}",
        math_glyphs.len()
    );
    assert!(
        math_glyphs
            .iter()
            .all(|(_, p, s)| p.x.is_finite() && p.y.is_finite() && s.is_finite())
    );
    assert!(
        text.contains("u_rest") && text.contains("$x^2$") && text.contains("let u_rest = -65;"),
        "code altered"
    );
    assert!(
        text.contains("\\unknowncommand{x}") && text.contains("$not_math$") && text.contains("$5")
    );
    assert!(text.contains('τ') && text.contains('∑') && text.contains('∫'));
    assert!(links > 0);
    let inline = all_glyphs
        .windows(6)
        .position(|w| w.iter().map(|(c, _)| *c).collect::<String>() == "Inline")
        .expect("inline marker");
    let h = all_glyphs[inline + 6..]
        .iter()
        .position(|(c, _)| *c == 'h')
        .unwrap()
        + inline
        + 6;
    let e = all_glyphs[h + 1..]
        .iter()
        .position(|(c, _)| *c == 'E')
        .unwrap()
        + h
        + 1;
    assert!(
        (all_glyphs[h].1.y - all_glyphs[e].1.y).abs() < 1.5,
        "native inline formula baseline differs from its text: {} vs {}",
        all_glyphs[h].1.y,
        all_glyphs[e].1.y
    );
    drop(book);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn plain_markdown_needs_no_math_conversion_and_formulas_recolor_on_demand() {
    let plain = Book::open(Path::new("tests/fixtures/reading.md")).unwrap();
    assert!(plain.conversion_style.is_none());
    let mut style = Typography::default();
    style.foreground = "#E7E9E5".into();
    style.background = "#171A19".into();
    let mut book = Book::open_styled(Path::new("tests/fixtures/math.md"), None, &style).unwrap();
    assert_eq!(book.conversion_style.as_ref().unwrap().0, style.foreground);
    book.layout(&style, 400.0, 500.0, 0).unwrap();
    let page = book.engine.load_page(0).unwrap();
    let pix = page
        .to_pixmap(
            &mupdf::Matrix::IDENTITY,
            &mupdf::Colorspace::device_rgb(),
            false,
            true,
        )
        .unwrap();
    assert_eq!(&pix.samples()[0..3], &[23, 26, 25]);
    if let Some(folder) = std::env::var_os("SAE_MATH_EVIDENCE_DIR") {
        std::fs::create_dir_all(&folder).unwrap();
        pix.save_as(
            Path::new(&folder).join("dark-narrow.png").to_str().unwrap(),
            mupdf::ImageFormat::PNG,
        )
        .unwrap();
    }
    assert!(
        !page.search("rest", 32).unwrap().is_empty(),
        "formula text no longer searchable"
    );
    drop(page);
    while !book.pagination_complete {
        book.paginate_next().unwrap();
    }
    let mut nominal_i = Vec::new();
    for number in 0..book.page_count {
        let page = book.engine.load_page(number).unwrap();
        let text = page.to_text_page(mupdf::TextPageFlags::empty()).unwrap();
        for block in text.blocks() {
            for line in block.lines() {
                for ch in line.chars() {
                    if ch.char() == Some('I') {
                        nominal_i.push(ch.size());
                    }
                }
            }
        }
        if number > 0 {
            if let Some(folder) = std::env::var_os("SAE_MATH_EVIDENCE_DIR") {
                page.to_pixmap(
                    &mupdf::Matrix::IDENTITY,
                    &mupdf::Colorspace::device_rgb(),
                    false,
                    true,
                )
                .unwrap()
                .save_as(
                    Path::new(&folder)
                        .join(format!("dark-narrow-{number}.png"))
                        .to_str()
                        .unwrap(),
                    mupdf::ImageFormat::PNG,
                )
                .unwrap();
            }
        }
    }
    assert!(nominal_i.len() >= 5);
    assert!(
        nominal_i.iter().all(|size| *size >= style.size_pt * 0.92),
        "formula was shrunk to the remaining page height: {nominal_i:?}"
    );
}
