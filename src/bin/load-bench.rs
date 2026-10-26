//! Privacy-preserving native load profiler. Never exports book text or images.
use anyhow::Result;
use mupdf::{Colorspace, Document, Matrix, TextPageFlags};
use sae_reader::{security, settings::Typography};
use serde_json::json;
use std::{io::Write, path::PathBuf, time::Instant};

fn stage<T>(name: &str, run: impl FnOnce() -> Result<T>) -> Result<T> {
    println!("{}", json!({"stage":name,"event":"begin"}));
    std::io::stdout().flush()?;
    let start = Instant::now();
    let value = run()?;
    println!(
        "{}",
        json!({"stage":name,"event":"end","ms":start.elapsed().as_secs_f64()*1000.0})
    );
    std::io::stdout().flush()?;
    Ok(value)
}
fn main() -> Result<()> {
    sae_reader::fonts::install();
    let path = PathBuf::from(std::env::args_os().nth(1).expect("path required"));
    if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("epub"))
    {
        stage("preflight", || security::preflight_epub(&path))?;
    }
    let mut doc = stage("native-open", || {
        Ok(Document::open(path.to_str().unwrap())?)
    })?;
    if doc.is_reflowable()? {
        let style = Typography::default();
        stage("style", || Ok(doc.set_style(&style.css(), true)?))?;
        stage("layout", || Ok(doc.layout(540.0, 540.0, style.size_pt)?))?;
    }
    stage("first-page", || {
        let page = doc.load_page(0)?;
        let pix = page.to_pixmap(
            &Matrix::new_scale(1.3333, 1.3333),
            &Colorspace::device_rgb(),
            false,
            true,
        )?;
        let text = page.to_text_page(TextPageFlags::empty())?.structured();
        let ink = pix
            .samples()
            .chunks_exact(3)
            .filter(|p| p[0].abs_diff(248) > 8 || p[1].abs_diff(246) > 8 || p[2].abs_diff(241) > 8)
            .count();
        println!(
            "{}",
            json!({"width":pix.width(),"height":pix.height(),"blocks":text.blocks.len(),"non_paper_pixels":ink})
        );
        Ok(())
    })?;
    let count = stage("full-page-count", || Ok(doc.page_count()?))?;
    let outlines = stage("resolved-contents", || Ok(doc.outlines()?.len()))?;
    println!("{}", json!({"pages":count,"outline_roots":outlines}));
    Ok(())
}
