use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Component, Path},
};

pub const MAX_FILE: u64 = 512 * 1024 * 1024;
const MAX_ENTRY: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;
const MAX_XML: u64 = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Debug, Default)]
pub struct EpubInspection {
    pub repaired_html: usize,
    pub ruby: usize,
    pub math: usize,
    pub vertical: bool,
    pub fixed: bool,
    pub viewport: Option<(f32, f32)>,
    pub complex_entries: Vec<String>,
    pub unconverted_complex_html: bool,
}
pub fn preflight_epub(path: &Path) -> Result<()> {
    inspect_epub(path).map(|_| ())
}
pub fn inspect_epub(path: &Path) -> Result<EpubInspection> {
    let file = File::open(path).context("Cannot open EPUB")?;
    ensure!(
        file.metadata()?.len() <= MAX_FILE,
        "EPUB exceeds the 512 MiB input limit"
    );
    let mut zip = zip::ZipArchive::new(file).context("Invalid EPUB ZIP archive")?;
    ensure!(zip.len() <= MAX_ENTRIES, "Too many EPUB resources");
    let mut total = 0u64;
    let mut report = EpubInspection::default();
    let mut names = HashSet::new();
    for i in 0..zip.len() {
        let mut item = zip
            .by_index(i)
            .context("Encrypted or unreadable EPUB resource")?;
        let name = item.name().to_owned();
        ensure!(safe_archive_path(&name), "Unsafe EPUB resource path");
        ensure!(
            names.insert(name.to_lowercase()),
            "Duplicate EPUB resource name"
        );
        ensure!(
            item.unix_mode()
                .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "EPUB symlinks are forbidden"
        );
        ensure!(
            item.size() <= MAX_ENTRY,
            "EPUB resource exceeds 32 MiB limit"
        );
        total = total
            .checked_add(item.size())
            .context("EPUB size overflow")?;
        ensure!(total <= MAX_TOTAL, "EPUB expansion exceeds 256 MiB limit");
        ensure!(
            item.size() <= 1024 * 1024 || item.size() / item.compressed_size().max(1) <= 250,
            "EPUB suspicious compression ratio"
        );
        let lower = name.to_lowercase();
        if [".xml", ".opf", ".xhtml", ".html", ".htm", ".ncx", ".svg"]
            .iter()
            .any(|s| lower.ends_with(s))
        {
            ensure!(
                item.size() <= MAX_XML,
                "EPUB XML resource exceeds 8 MiB limit"
            );
            let mut bytes = Vec::with_capacity(item.size() as usize);
            item.by_ref().take(MAX_XML + 1).read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() as u64 <= MAX_XML,
                "EPUB actual XML size exceeds limit"
            );
            let html = [".xhtml", ".html", ".htm"]
                .iter()
                .any(|ext| lower.ends_with(ext));
            let before = (report.ruby, report.math);
            if validate_markup_inspected(&bytes, html, Some(&mut report))? {
                report.repaired_html += 1;
            }
            if html && before != (report.ruby, report.math) {
                report.complex_entries.push(name);
            }
        } else if lower.ends_with(".css") && item.size() <= MAX_XML {
            let mut css = String::new();
            if item.read_to_string(&mut css).is_ok() {
                report.vertical |= css.contains("vertical-rl") || css.contains("vertical-lr");
            }
        }
    }
    ensure!(
        names.contains("meta-inf/container.xml"),
        "EPUB container.xml is missing"
    );
    let mut mimetype = String::new();
    zip.by_name("mimetype")
        .context("EPUB mimetype is missing")?
        .take(128)
        .read_to_string(&mut mimetype)?;
    ensure!(
        mimetype.trim() == "application/epub+zip",
        "Invalid EPUB mimetype"
    );
    Ok(report)
}

pub fn safe_archive_path(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', ':', '\0'])
        && !name.starts_with('/')
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !name.split('/').any(|part| part == ".." || part == ".")
}

#[cfg(test)]
fn validate_xml(bytes: &[u8]) -> Result<()> {
    validate_markup(bytes, false).map(|_| ())
}
#[cfg(test)]
fn validate_markup(bytes: &[u8], html: bool) -> Result<bool> {
    validate_markup_inspected(bytes, html, None)
}
fn validate_markup_inspected(
    bytes: &[u8],
    html: bool,
    mut report: Option<&mut EpubInspection>,
) -> Result<bool> {
    let (decoded, had_errors) = if bytes.starts_with(&[0xff, 0xfe]) {
        encoding_rs::UTF_16LE.decode_without_bom_handling(&bytes[2..])
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        encoding_rs::UTF_16BE.decode_without_bom_handling(&bytes[2..])
    } else {
        encoding_rs::UTF_8
            .decode_without_bom_handling(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
    };
    ensure!(!had_errors, "Invalid EPUB XML character encoding");
    let text = decoded.as_ref();
    // roxmltree never resolves an external DTD. Common EPUB2 PUBLIC DOCTYPEs
    // are accepted, but declared entities are forbidden before either parser.
    let upper = text.to_ascii_uppercase();
    ensure!(
        !upper.contains("<!ENTITY"),
        "XML entity declarations are forbidden"
    );
    let parsed = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 300_000,
        },
    );
    if html && parsed.is_err() {
        validate_repairable_html(text)?;
        if let Some(report) = report.as_deref_mut() {
            report.unconverted_complex_html |= text.contains("<ruby") || text.contains("<math");
        }
        return Ok(true);
    }
    let doc = parsed.context("Malformed EPUB XML")?;
    for node in doc.descendants() {
        ensure!(
            node.ancestors().take(129).count() <= 128,
            "EPUB XML nesting exceeds limit"
        );
        if node.is_element() && node.tag_name().name() == "script" {
            bail!("Scripted EPUB content is unsupported; no script is executed");
        }
        if let Some(report) = report.as_deref_mut() {
            if node.is_element() {
                match node.tag_name().name() {
                    "ruby" => report.ruby += 1,
                    "math" => report.math += 1,
                    "meta" => {
                        if node.attribute("property") == Some("rendition:layout")
                            && node.text().is_some_and(|s| s.trim() == "pre-paginated")
                        {
                            report.fixed = true;
                        }
                        if node.attribute("name") == Some("viewport") && report.viewport.is_none() {
                            let mut w = None;
                            let mut h = None;
                            for field in node.attribute("content").unwrap_or("").split([',', ';']) {
                                if let Some((name, value)) = field.trim().split_once('=') {
                                    if let Ok(value) = value.trim().parse::<f32>() {
                                        if value.is_finite() && (16.0..=20000.0).contains(&value) {
                                            match name.trim() {
                                                "width" => w = Some(value),
                                                "height" => h = Some(value),
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            if let (Some(w), Some(h)) = (w, h) {
                                // This pinned MuPDF HTML engine treats CSS px as
                                // one layout unit. Keep viewport and source px in
                                // that same coordinate space for fixed canvases.
                                report.viewport = Some((w, h));
                            }
                        }
                    }
                    _ => {}
                }
                report.vertical |= node
                    .attribute("style")
                    .is_some_and(|s| s.contains("vertical-rl") || s.contains("vertical-lr"));
            }
        }
    }
    Ok(false)
}

// Bound malformed HTML before allowing MuPDF's native HTML5 recovery parser.
// Do not repair archive bytes or weaken OPF/container/NCX/SVG XML validation.
fn validate_repairable_html(text: &str) -> Result<()> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut stack: Vec<String> = Vec::new();
    let mut nodes = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if text[i..].starts_with("<!--") {
            i += text[i + 4..].find("-->").context("Unclosed HTML comment")? + 7;
            continue;
        }
        let start = i;
        i += 1;
        let mut quote = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if quote != 0 {
                if b == quote {
                    quote = 0;
                }
            } else if b == b'\'' || b == b'"' {
                quote = b;
            } else if b == b'>' {
                break;
            }
            i += 1;
        }
        ensure!(i < bytes.len(), "Unclosed HTML tag");
        let raw = text[start + 1..i].trim();
        i += 1;
        if raw.starts_with(['!', '?']) {
            continue;
        }
        let closing = raw.starts_with('/');
        let tag = raw
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        ensure!(
            tag != "script",
            "Scripted EPUB content is unsupported; no script is executed"
        );
        nodes += 1;
        ensure!(nodes <= 300_000, "Too many HTML nodes");
        if closing {
            if let Some(pos) = stack.iter().rposition(|s| *s == tag) {
                stack.truncate(pos);
            }
        } else if !raw.ends_with('/')
            && !matches!(
                tag.as_str(),
                "br" | "hr"
                    | "img"
                    | "meta"
                    | "link"
                    | "input"
                    | "area"
                    | "base"
                    | "col"
                    | "embed"
                    | "param"
                    | "source"
                    | "track"
                    | "wbr"
            )
        {
            if matches!(tag.as_str(), "p" | "li" | "dt" | "dd")
                && stack.last().is_some_and(|s| *s == tag)
            {
                stack.pop();
            }
            stack.push(tag);
            ensure!(stack.len() <= 128, "EPUB HTML nesting exceeds limit");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hostile_paths_and_expansion_are_rejected_before_engine() {
        for name in ["../x", "C:/x", "/root", "a\\b", "a/../../x", "a/./x"] {
            assert!(!safe_archive_path(name), "{name}");
        }
        for fixture in ["hostile-traversal.epub", "hostile-ratio.epub"] {
            assert!(preflight_epub(&Path::new("tests/fixtures").join(fixture)).is_err());
        }
        preflight_epub(Path::new("tests/fixtures/core-epub3.epub")).unwrap();
    }
    #[test]
    fn entities_and_deep_xml_are_rejected() {
        assert!(
            validate_xml(b"<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///secret'>]><a>&x;</a>").is_err()
        );
        let deep = format!("{}{}", "<a>".repeat(130), "</a>".repeat(130));
        assert!(validate_xml(deep.as_bytes()).is_err());
    }
    #[test]
    fn malformed_book_html_is_recoverable_without_allowing_entities_or_depth_bombs() {
        assert!(
            validate_markup(b"<html><body><div>Line<br>Next</div></body></html>", true).unwrap()
        );
        assert!(
            validate_markup(b"<html><body><div>Line<br>Next</div></body></html>", false).is_err()
        );
        assert!(
            validate_markup(
                b"<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///secret'>]><br>",
                true
            )
            .is_err()
        );
        assert!(
            validate_markup(
                format!("{}x{}", "<div>".repeat(130), "</div>".repeat(130)).as_bytes(),
                true
            )
            .is_err()
        );
    }
}
