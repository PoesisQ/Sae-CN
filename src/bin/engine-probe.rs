use anyhow::{Result, ensure};
use mupdf::{
    Colorspace, Document, ImageFormat, Matrix, MetadataName, TextBlockContent, TextExtractOptions,
    TextPageFlags,
};
use sae_reader::{security::preflight_epub, settings::Typography};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

fn normalize(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn page_evidence(doc: &Document, i: i32, out: &str) -> Result<Value> {
    let t = Instant::now();
    let page = doc.load_page(i)?;
    let pix = page.to_pixmap(&Matrix::IDENTITY, &Colorspace::device_rgb(), false, true)?;
    pix.save_as(
        format!("{out}/page-{:03}.png", i + 1).as_str(),
        ImageFormat::PNG,
    )?;
    let text = page.text(TextExtractOptions::default())?;
    let structured = page
        .to_text_page(TextPageFlags::PRESERVE_IMAGES)?
        .structured();
    let mut sizes = Vec::new();
    let mut images = 0;
    let mut glyphs = 0;
    for block in &structured.blocks {
        match &block.content {
            TextBlockContent::Text { lines } => {
                for line in lines {
                    for ch in &line.chars {
                        sizes.push(ch.size);
                        glyphs += 1;
                    }
                }
            }
            TextBlockContent::Image { .. } => images += 1,
            _ => {}
        }
    }
    let links: Vec<_> = page.links()?.map(|l| json!({"uri":l.uri,"bounds":[l.bounds.x0,l.bounds.y0,l.bounds.x1,l.bounds.y1],"page":l.dest.map(|d|d.loc.page_number)})).collect();
    let hits = page.search("Paragraph", 64)?;
    let search_bounds: Vec<_> = hits
        .iter()
        .map(|q| json!([q.ul.x, q.ul.y, q.lr.x, q.lr.y]))
        .collect();
    let green_pixels = pix
        .samples()
        .chunks_exact(pix.n() as usize)
        .filter(|p| {
            p.len() >= 3 && p[0].abs_diff(80) < 3 && p[1].abs_diff(108) < 3 && p[2].abs_diff(96) < 3
        })
        .count();
    std::fs::write(format!("{out}/page-{:03}.txt", i + 1), &text)?;
    Ok(
        json!({"page":i,"ms":t.elapsed().as_secs_f64()*1000.0,"text_characters":text.chars().count(),"glyphs":glyphs,"native_image_blocks":images,"fixture_green_pixels":green_pixels,"font_sizes":sizes.into_iter().take(8).collect::<Vec<_>>(),"links":links,"search_bounds":search_bounds,"width":pix.width(),"height":pix.height(),"pixel_blake3":blake3::hash(pix.samples()).to_hex().as_str()}),
    )
}

fn main() -> Result<()> {
    sae_reader::fonts::install();
    let args: Vec<String> = std::env::args().collect();
    let input = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("tests/fixtures/core-epub3.epub");
    let out = args.get(2).map(String::as_str).unwrap_or("artifacts/probe");
    std::fs::create_dir_all(out)?;
    if input.to_lowercase().ends_with(".epub") {
        preflight_epub(Path::new(input))?;
    }
    let original_hash = blake3::hash(&std::fs::read(input)?);
    let start = Instant::now();
    if args.iter().any(|a| a == "--reader") {
        let mut book = sae_reader::document::Book::open(Path::new(input))?;
        book.layout(&Typography::default(), 540.0, 600.0, 0)?;
        while !book.pagination_complete {
            book.paginate_next()?;
        }
        let mut pages = Vec::new();
        for number in 0..book.page_count.min(20) {
            let page = book.engine.load_page(number)?;
            pages.push(page_evidence(&book.engine, number, out)?);
            let structured = page
                .to_text_page(TextPageFlags::PRESERVE_IMAGES)?
                .structured();
            std::fs::write(
                format!("{out}/structure-{number}.txt"),
                format!("{structured:#?}"),
            )?;
        }
        std::fs::write(
            format!("{out}/reader-report.json"),
            serde_json::to_vec_pretty(
                &json!({"pages":pages,"count":book.page_count,"reflowable":book.reflowable,"notice":book.notice}),
            )?,
        )?;
        return Ok(());
    }
    let mut doc = Document::open(input)?;
    let reflow = doc.is_reflowable()?;
    let base = Typography::default();
    if reflow {
        doc.set_style(&base.css(), true)?;
        doc.layout(600.0, 800.0, base.size_pt)?;
    }
    let pages = doc.page_count()?;
    let baseline_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut rows = Vec::new();
    for i in 0..pages.min(20) {
        rows.push(page_evidence(&doc, i, out)?);
    }
    let mut variants = Vec::new();
    if reflow && pages > 10 {
        let anchor_page = 8;
        let anchor = doc.logical_anchor(anchor_page)?;
        ensure!(
            anchor >> 32 > 0,
            "EPUB anchor is an absolute page, not a chapter/flow reference"
        );
        let old_text = normalize(
            &doc.load_page(anchor_page)?
                .text(TextExtractOptions::default())?,
        );
        let needle: String = old_text.chars().take(24).collect();
        for name in ["size", "line", "paragraph", "width", "margin"] {
            let mut style = base.clone();
            let width = if name == "width" { 420.0 } else { 600.0 };
            match name {
                "size" => style.size_pt = 26.0,
                "line" => style.line_height = 2.05,
                "paragraph" => style.paragraph_after = 1.8,
                "margin" => style.margins = [72.0, 64.0, 72.0, 64.0],
                _ => {}
            }
            let t = Instant::now();
            doc.set_style(&style.css(), true)?;
            doc.layout(width, 800.0, style.size_pt)?;
            let new_pages = doc.page_count()?;
            let resolved = doc
                .resolve_anchor(anchor)?
                .expect("anchor must resolve after layout");
            let new_text = normalize(
                &doc.load_page(resolved)?
                    .text(TextExtractOptions::default())?,
            );
            let preserved = new_text.contains(&needle);
            let variant_dir = format!("{out}/{name}");
            std::fs::create_dir_all(&variant_dir)?;
            let page = page_evidence(&doc, resolved, &variant_dir)?;
            let first = doc.load_page(0)?.to_pixmap(
                &Matrix::IDENTITY,
                &Colorspace::device_rgb(),
                false,
                true,
            )?;
            first.save_as(
                format!("{variant_dir}/first-page.png").as_str(),
                ImageFormat::PNG,
            )?;
            let mut reopened = Document::open(input)?;
            reopened.set_style(&style.css(), true)?;
            reopened.layout(width, 800.0, style.size_pt)?;
            let reopen_page = reopened.resolve_anchor(anchor)?;
            variants.push(json!({"setting":name,"page_count":new_pages,"before_page":anchor_page,"after_page":resolved,"reopened_page":reopen_page,"same_source_text_visible":preserved,"ms":t.elapsed().as_secs_f64()*1000.0,"page_evidence":page}));
            ensure!(
                new_pages != pages,
                "{name} did not alter real pagination ({pages} pages)"
            );
            ensure!(preserved, "{name} lost the source anchor text");
            ensure!(
                reopen_page == Some(resolved),
                "{name} anchor changed after reopening"
            );
        }
    }
    ensure!(
        blake3::hash(&std::fs::read(input)?) == original_hash,
        "Original input was modified"
    );
    let report = json!({"binding":"mupdf 0.8.0 + documented local extensions","native_engine":"MuPDF 1.27.2","input_sample":Path::new(input).file_name().unwrap().to_string_lossy(),"title":doc.metadata(MetadataName::Title)?,"baseline_page_count":pages,"baseline_open_layout_count_ms":baseline_ms,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"original_unchanged":true,"outline_roots":doc.outlines()?.len(),"pages":rows,"layout_variants":variants});
    std::fs::write(
        format!("{out}/report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "{}: {} baseline pages, {} rendered, {} layout variants, {:.1} ms total",
        input,
        pages,
        report["pages"].as_array().unwrap().len(),
        report["layout_variants"].as_array().unwrap().len(),
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
