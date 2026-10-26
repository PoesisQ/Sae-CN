//! On-demand text/Markdown adapters. Original files are never rewritten.
//! A short-lived EPUB uses the same native layout, links and selection engine.
use anyhow::{Context, Result, bail, ensure};
use encoding_rs::{CoderResult, Encoding};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use zip::{ZipWriter, write::SimpleFileOptions};

pub struct Converted {
    pub path: PathBuf,
    pub encoding: String,
    pub notice: String,
    directory: PathBuf,
}
impl Drop for Converted {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}
impl Converted {
    pub(crate) fn temporary(kind: &str) -> Result<Self> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "sae-{kind}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)?;
        Ok(Self {
            path: directory.join("reading.epub"),
            directory,
            encoding: String::new(),
            notice: String::new(),
        })
    }
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn url_path(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let pair = bytes.get(i + 1..i + 3)?;
            let n = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            decoded.push(n(pair[0])? * 16 + n(pair[1])?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}
fn entry(zip: &mut ZipWriter<File>, name: &str, data: &[u8]) -> Result<()> {
    zip.start_file(
        name,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    zip.write_all(data)?;
    Ok(())
}
fn chapter(zip: &mut ZipWriter<File>, number: usize, body: &str) -> Result<()> {
    let page = format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>{number}</title><link rel=\"stylesheet\" href=\"style.css\"/></head><body>{body}</body></html>"
    );
    entry(zip, &format!("c{number}.xhtml"), page.as_bytes())
}
fn encoding(sample: &[u8], selected: Option<&str>) -> Result<&'static Encoding> {
    if let Some(selected) = selected {
        return Encoding::for_label(selected.as_bytes()).context("不支持此文本编码");
    }
    if let Some((encoding, _)) = Encoding::for_bom(sample) {
        return Ok(encoding);
    }
    // BOM-less UTF-16 often contains alternating NUL bytes, which a legacy
    // Chinese detector otherwise treats as valid ASCII.
    if sample.len() >= 8 {
        let pairs = sample.chunks_exact(2);
        let count = pairs.len();
        let (mut even, mut odd) = (0, 0);
        for p in pairs {
            even += (p[0] == 0) as usize;
            odd += (p[1] == 0) as usize;
        }
        if odd * 3 > count && even * 20 < count {
            return Ok(encoding_rs::UTF_16LE);
        }
        if even * 3 > count && odd * 20 < count {
            return Ok(encoding_rs::UTF_16BE);
        }
    }
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(sample, sample.len() < 65536);
    Ok(detector.guess(Some(b"cn"), true))
}
fn decode(
    mut input: impl Read,
    encoding: &'static Encoding,
    mut consume: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut decoder = encoding.new_decoder_with_bom_removal();
    let mut bytes = [0u8; 32768];
    loop {
        let len = input.read(&mut bytes)?;
        let mut rest = &bytes[..len];
        loop {
            let mut text = String::with_capacity(131072);
            let (status, read, errors) = decoder.decode_to_string(rest, &mut text, len == 0);
            ensure!(
                !errors,
                "文本含无法按 {} 解码的字节，请在工具栏手动切换编码；原文件未改动",
                encoding.name()
            );
            consume(&text)?;
            rest = &rest[read..];
            if status == CoderResult::InputEmpty {
                break;
            }
        }
        if len == 0 {
            break;
        }
    }
    Ok(())
}

pub fn convert(path: &Path, selected: Option<&str>) -> Result<Converted> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let mut file = File::open(path)?;
    let mut prefix = vec![0; 65536];
    let len = file.read(&mut prefix)?;
    prefix.truncate(len);
    let encoding = encoding(&prefix, selected)?;
    let directory = std::env::temp_dir().join(format!(
        "sae-text-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory)?;
    let mut converted = Converted {
        path: directory.join("reading.epub"),
        directory,
        encoding: encoding.name().to_owned(),
        notice: String::new(),
    };
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(&converted.path)?;
    let mut zip = ZipWriter::new(output);
    entry(&mut zip, "mimetype", b"application/epub+zip")?;
    entry(&mut zip, "META-INF/container.xml", b"<container xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\" version=\"1.0\"><rootfiles><rootfile full-path=\"book.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>")?;
    entry(&mut zip, "style.css", b"p.txtline{white-space:pre-wrap}pre{white-space:pre-wrap;overflow-wrap:anywhere;border:1px solid #8c988e;padding:.6em;font-size:.9em}code{font-family:monospace}table{border-collapse:collapse}td,th{border:1px solid #8c988e;padding:.35em}blockquote{margin-left:1.5em;border-left:2px solid #8c988e;padding-left:.7em}img{max-width:100%;height:auto}h1,h2,h3{text-indent:0}h1{font-size:1.45em;line-height:1.25;margin-top:0;margin-bottom:.6em}h2{font-size:1.2em;line-height:1.3}h3{font-size:1.05em;line-height:1.3}hr{border-top:1px solid #8c988e}")?;
    let input = std::io::Cursor::new(prefix).chain(file);
    let mut chapters = 0;
    let mut nav = String::new();
    let mut assets = Vec::new();
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    {
        let mut text = String::new();
        decode(input, encoding, |part| {
            text.push_str(part);
            Ok(())
        })?;
        let mut headings = 0;
        let mut heading_title = None::<String>;
        let mut heading_ids = HashMap::<String, String>::new();
        let mut resource_cache = HashMap::<String, Option<String>>::new();
        let base = path.parent().unwrap_or(Path::new(".")).canonicalize()?;
        let mut blocked = 0;
        let mut raw_html = false;
        let mut image_stack = Vec::new();
        let mut total_assets = 0u64;
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_HEADING_ATTRIBUTES;
        // Allocate stable ids once, including duplicates, before resolving links.
        for event in Parser::new_ext(&text, options) {
            match event {
                Event::Start(Tag::Heading { id, .. }) => {
                    headings += 1;
                    heading_title = Some(String::new());
                    if let Some(id) = id {
                        heading_ids.insert(id.to_string(), format!("heading-{headings}"));
                    }
                }
                Event::Text(t) | Event::Code(t) => {
                    if let Some(title) = &mut heading_title {
                        title.push_str(&t);
                    }
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some(title) = heading_title.take() {
                        let slug = title
                            .to_lowercase()
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join("-");
                        heading_ids
                            .entry(slug)
                            .or_insert_with(|| format!("heading-{headings}"));
                        nav.push_str(&format!(
                            "<li><a href=\"c1.xhtml#heading-{headings}\">{}</a></li>",
                            escape(&title)
                        ));
                    }
                }
                _ => {}
            }
        }
        headings = 0;
        let events = Parser::new_ext(&text, options).map(|event| match event {
            Event::Html(value) | Event::InlineHtml(value) => {
                raw_html = true;
                Event::Text(value)
            }
            Event::Start(Tag::Heading { level, .. }) => {
                headings += 1;
                Event::Start(Tag::Heading {
                    level,
                    id: Some(format!("heading-{headings}").into()),
                    classes: Vec::new(),
                    attrs: Vec::new(),
                })
            }
            Event::Start(Tag::Link {
                link_type,
                mut dest_url,
                title,
                id,
            }) => {
                if let Some(fragment) = dest_url.strip_prefix('#') {
                    if let Some(actual) =
                        heading_ids.get(url_path(fragment).as_deref().unwrap_or(fragment))
                    {
                        dest_url = format!("#{actual}").into();
                    }
                } else if !(dest_url.starts_with("https://") || dest_url.starts_with("http://")) {
                    blocked += 1;
                    dest_url = "#blocked-link".into();
                }
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let resource = resource_cache
                    .entry(dest_url.to_string())
                    .or_insert_with(|| {
                        let safe = (|| -> Result<String> {
                            let decoded = url_path(&dest_url).context("图片地址编码无效")?;
                            ensure!(
                                !decoded.contains(':')
                                    && !decoded.starts_with(['/', '\\'])
                                    && !decoded.contains(['?', '#']),
                                "外部图片"
                            );
                            let local = base.join(&decoded).canonicalize()?;
                            ensure!(local.starts_with(&base), "图片越出文档目录");
                            let ext = local
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_ascii_lowercase();
                            let media = match ext.as_str() {
                                "png" => "image/png",
                                "jpg" | "jpeg" => "image/jpeg",
                                "gif" => "image/gif",
                                "webp" => "image/webp",
                                "bmp" => "image/bmp",
                                _ => bail!("未支持的图片格式"),
                            };
                            let size = local.metadata()?.len();
                            ensure!(
                                size <= 32 * 1024 * 1024
                                    && total_assets + size <= 128 * 1024 * 1024,
                                "图片超过安全上限"
                            );
                            let name = format!("image-{}.{}", assets.len(), ext);
                            entry(&mut zip, &name, &fs::read(local)?)?;
                            total_assets += size;
                            assets.push((name.clone(), media));
                            Ok(name)
                        })();
                        if safe.is_err() {
                            blocked += 1;
                        }
                        safe.ok()
                    })
                    .clone();
                image_stack.push(resource.is_some());
                if let Some(dest_url) = resource {
                    Event::Start(Tag::Image {
                        link_type,
                        dest_url: dest_url.into(),
                        title,
                        id,
                    })
                } else {
                    Event::Start(Tag::Emphasis)
                }
            }
            Event::End(TagEnd::Image) => {
                if image_stack.pop().unwrap_or(false) {
                    Event::End(TagEnd::Image)
                } else {
                    Event::End(TagEnd::Emphasis)
                }
            }
            other => other,
        });
        let mut body = String::new();
        html::push_html(&mut body, events);
        chapter(&mut zip, 1, &body)?;
        chapters = 1;
        converted.notice = format!(
            "Markdown · {} · 代码块自动换行{}{}",
            encoding.name(),
            if raw_html {
                " · 原始 HTML 按文字显示"
            } else {
                ""
            },
            if blocked > 0 {
                format!(" · 已阻止 {blocked} 个外部或越界资源")
            } else {
                String::new()
            }
        );
    } else {
        let mut pending = String::new();
        let mut body = String::new();
        let mut line_number = 0;
        let mut write_line = |line: &str, zip: &mut ZipWriter<File>| -> Result<()> {
            if body.len() > 65536 {
                chapters += 1;
                chapter(zip, chapters, &body)?;
                body.clear();
            }
            line_number += 1;
            if body.is_empty() {
                nav.push_str(&format!(
                    "<li><a href=\"c{}.xhtml\">第 {line_number} 行</a></li>",
                    chapters + 1
                ));
            }
            body.push_str("<p class=\"txtline\">");
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.is_empty() {
                body.push_str("&#160;");
            } else {
                body.push_str(&escape(line));
            }
            body.push_str("</p>");
            Ok(())
        };
        decode(input, encoding, |part| {
            pending.push_str(part);
            let mut start = 0;
            for (end, _) in pending.match_indices('\n') {
                write_line(&pending[start..end], &mut zip)?;
                start = end + 1;
            }
            pending.drain(..start);
            ensure!(
                pending.len() <= 8 * 1024 * 1024,
                "单行文本超过 8 MiB 安全上限"
            );
            Ok(())
        })?;
        if !pending.is_empty() {
            write_line(&pending, &mut zip)?;
        }
        drop(write_line);
        chapters += 1;
        chapter(&mut zip, chapters, &body)?;
        converted.notice = format!("TXT · {} · 保留空行与行首缩进", encoding.name());
    }
    if nav.is_empty() {
        nav = "<li><a href=\"c1.xhtml\">正文</a></li>".to_owned();
    }
    entry(&mut zip,"nav.xhtml",format!("<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\"><head><title>目录</title></head><body><nav epub:type=\"toc\"><ol>{nav}</ol></nav></body></html>").as_bytes())?;
    let manifest = (1..=chapters)
        .map(|i| {
            format!("<item id=\"c{i}\" href=\"c{i}.xhtml\" media-type=\"application/xhtml+xml\"/>")
        })
        .chain(assets.iter().enumerate().map(|(i, (name, media))| {
            format!("<item id=\"image{i}\" href=\"{name}\" media-type=\"{media}\"/>")
        }))
        .collect::<String>();
    let spine = (1..=chapters)
        .map(|i| format!("<itemref idref=\"c{i}\"/>"))
        .collect::<String>();
    entry(&mut zip,"book.opf",format!("<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"uid\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"uid\">sae-local-text</dc:identifier><dc:language>zh</dc:language></metadata><manifest>{manifest}<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"css\" href=\"style.css\" media-type=\"text/css\"/></manifest><spine>{spine}</spine></package>").as_bytes())?;
    zip.finish()?;
    Ok(converted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoding_is_incremental_and_never_silently_replaces_malformed_bytes() {
        let mut out = String::new();
        decode(
            &b"\xff\xfe\x22\x6f\x57\x5b"[..],
            encoding_rs::UTF_16LE,
            |p| {
                out.push_str(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(out, "漢字");
        assert!(decode(&b"bad\xff"[..], encoding_rs::UTF_8, |_| Ok(())).is_err());
    }
}
