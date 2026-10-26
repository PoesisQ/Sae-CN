//! Targeted native EPUB fallback, allocated only for books containing these tags.
//! Keep every original ZIP resource and replace supported inline structures with
//! vector SVG. Unsupported structures remain intact and are reported explicitly.
use crate::{security::EpubInspection, settings::Typography, text_adapter::Converted};
use anyhow::{Result, ensure};
use roxmltree::Node;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn text(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect()
}
fn width(value: &str) -> f32 {
    value
        .chars()
        .map(|c| {
            if c as u32 >= 0x2e80 {
                32.0
            } else if c.is_whitespace() {
                9.0
            } else {
                20.0
            }
        })
        .sum::<f32>()
        .max(4.0)
}
struct Boxed {
    w: f32,
    h: f32,
    base: f32,
    svg: String,
}
impl Boxed {
    fn translate(&self, x: f32, y: f32, scale: f32) -> String {
        format!(
            "<g transform=\"translate({x} {y}) scale({scale})\">{}</g>",
            self.svg
        )
    }
    fn leaf(value: &str) -> Self {
        Self {
            w: width(value),
            h: 40.0,
            base: 30.0,
            svg: format!(
                "<text x=\"0\" y=\"30\" font-size=\"32\">{}</text>",
                xml(value)
            ),
        }
    }
}
fn group(children: Vec<Boxed>) -> Boxed {
    let base = children.iter().map(|b| b.base).fold(0.0, f32::max);
    let below = children.iter().map(|b| b.h - b.base).fold(0.0, f32::max);
    let mut x = 0.0;
    let mut svg = String::new();
    for child in children {
        svg.push_str(&child.translate(x, base - child.base, 1.0));
        x += child.w + 2.0;
    }
    Boxed {
        w: x.max(4.0),
        h: base + below,
        base,
        svg,
    }
}
fn math(node: Node<'_, '_>, depth: usize) -> Option<Boxed> {
    if depth > 24 {
        return None;
    }
    let tag = node.tag_name().name();
    if matches!(tag, "mi" | "mn" | "mo" | "mtext" | "ms") {
        return Some(Boxed::leaf(&text(node)));
    }
    let nodes = node.children().filter(Node::is_element).collect::<Vec<_>>();
    let mut children = Vec::new();
    for child in nodes {
        children.push(math(child, depth + 1)?);
    }
    match tag {
        "math" | "mrow" | "mstyle" => Some(group(children)),
        "mfrac" if children.len() == 2 => {
            let a = &children[0];
            let b = &children[1];
            let scale = 0.85;
            let w = a.w.max(b.w) * scale + 12.0;
            let rule = a.h * scale + 4.0;
            let bottom = rule + 8.0;
            Some(Boxed {
                w,
                h: bottom + b.h * scale,
                base: rule + 12.0,
                svg: format!(
                    "{}<path d=\"M1 {rule} H{}\" stroke=\"currentColor\" stroke-width=\"1.5\"/>{}",
                    a.translate((w - a.w * scale) / 2.0, 0.0, scale),
                    w - 1.0,
                    b.translate((w - b.w * scale) / 2.0, bottom, scale)
                ),
            })
        }
        "msup" | "msub" if children.len() == 2 => {
            let a = &children[0];
            let b = &children[1];
            let scale = 0.65;
            let ay = if tag == "msup" {
                (b.h * scale - 10.0).max(10.0)
            } else {
                0.0
            };
            let by = if tag == "msup" { 0.0 } else { a.base - 3.0 };
            Some(Boxed {
                w: a.w + b.w * scale,
                h: (ay + a.h).max(by + b.h * scale),
                base: ay + a.base,
                svg: format!(
                    "{}{}",
                    a.translate(0.0, ay, 1.0),
                    b.translate(a.w, by, scale)
                ),
            })
        }
        "msubsup" if children.len() == 3 => {
            let a = &children[0];
            let sub = &children[1];
            let sup = &children[2];
            let ay = (sup.h * 0.65 - 10.0).max(10.0);
            Some(Boxed {
                w: a.w + sub.w.max(sup.w) * 0.65,
                h: (ay + a.h).max(ay + a.base + sub.h * 0.65 - 3.0),
                base: ay + a.base,
                svg: format!(
                    "{}{}{}",
                    a.translate(0.0, ay, 1.0),
                    sub.translate(a.w, ay + a.base - 3.0, 0.65),
                    sup.translate(a.w, 0.0, 0.65)
                ),
            })
        }
        "msqrt" => {
            let a = group(children);
            let h = a.h + 5.0;
            let base = a.base + 5.0;
            let w = a.w + 24.0;
            Some(Boxed {
                w,
                h,
                base,
                svg: format!(
                    "<path d=\"M1 {} L7 {} L13 {} L21 2 H{w}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.6\"/>{}",
                    h * 0.55,
                    h * 0.50,
                    h - 2.0,
                    a.translate(24.0, 5.0, 1.0)
                ),
            })
        }
        "mfenced" => {
            let mut result = vec![Boxed::leaf(node.attribute("open").unwrap_or("("))];
            for (i, child) in children.into_iter().enumerate() {
                if i > 0 {
                    result.push(Boxed::leaf(","));
                }
                result.push(child);
            }
            result.push(Boxed::leaf(node.attribute("close").unwrap_or(")")));
            Some(group(result))
        }
        _ => None,
    }
}
fn ruby(node: Node<'_, '_>) -> Option<Boxed> {
    let mut base = String::new();
    let mut pairs = Vec::new();
    for child in node.children() {
        if child.is_text() {
            base.push_str(child.text().unwrap_or(""));
            continue;
        }
        if !child.is_element() {
            continue;
        }
        match child.tag_name().name() {
            "rp" => {}
            "rt" => {
                if base.is_empty() {
                    return None;
                }
                pairs.push((std::mem::take(&mut base), text(child)));
            }
            "rb" => base.push_str(&text(child)),
            _ => return None,
        }
    }
    if !base.is_empty() {
        pairs.push((base, String::new()));
    }
    if pairs.is_empty() {
        return None;
    }
    let mut x = 0.0;
    let mut base_svg = String::new();
    let mut annotation_svg = String::new();
    for (base, annotation) in pairs {
        let bw = width(&base);
        let aw = width(&annotation) * 0.5;
        let w = bw.max(aw);
        base_svg.push_str(&format!(
            "<text x=\"{}\" y=\"51\" font-size=\"32\">{}</text>",
            (w - bw) / 2.0 + x,
            xml(&base)
        ));
        annotation_svg.push_str(&format!(
            "<text x=\"{}\" y=\"17\" font-size=\"16\">{}</text>",
            (w - aw) / 2.0 + x,
            xml(&annotation)
        ));
        x += w + 2.0;
    }
    let svg = base_svg + &annotation_svg;
    Some(Boxed {
        w: x,
        h: 59.0,
        base: 51.0,
        svg,
    })
}
pub fn convert(
    path: &std::path::Path,
    report: &EpubInspection,
    style: &Typography,
) -> Result<Option<Converted>> {
    if report.complex_entries.is_empty() {
        return Ok(None);
    }
    let mut result = Converted::temporary("epub-compat")?;
    let mut source = ZipArchive::new(File::open(path)?)?;
    let output = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&result.path)?;
    let mut target = ZipWriter::new(output);
    let (mut ruby_count, mut math_count, mut unsupported) = (0, 0, 0);
    for i in 0..source.len() {
        let mut item = source.by_index(i)?;
        let name = item.name().to_owned();
        if report.complex_entries.contains(&name) {
            target.start_file(
                &name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )?;
            let mut bytes = Vec::new();
            item.read_to_end(&mut bytes)?;
            let (encoding, skip) =
                encoding_rs::Encoding::for_bom(&bytes).unwrap_or((encoding_rs::UTF_8, 0));
            let (text, errors) = encoding.decode_without_bom_handling(&bytes[skip..]);
            ensure!(!errors, "兼容转换的字符编码无效");
            let mut text = text.into_owned();
            if text.starts_with("<?xml") {
                if let Some(end) = text.find("?>") {
                    text.drain(..end + 2);
                }
            }
            let doc = roxmltree::Document::parse_with_options(
                &text,
                roxmltree::ParsingOptions {
                    allow_dtd: true,
                    nodes_limit: 300_000,
                },
            )?;
            let mut replacements = Vec::new();
            for node in doc.descendants().filter(Node::is_element) {
                let tag = node.tag_name().name();
                if !matches!(tag, "math" | "ruby")
                    || node
                        .ancestors()
                        .skip(1)
                        .any(|a| a.is_element() && matches!(a.tag_name().name(), "math" | "ruby"))
                {
                    continue;
                }
                let layout = if node.descendants().take(1025).count() > 1024 {
                    None
                } else if tag == "math" {
                    math(node, 0)
                } else {
                    ruby(node)
                };
                if let Some(layout) = layout.filter(|b| {
                    b.w.is_finite()
                        && b.h.is_finite()
                        && b.w <= 10000.0
                        && b.svg.len() <= 128 * 1024
                }) {
                    let family = if tag == "math" {
                        "Times New Roman"
                    } else if style.family == "serif" {
                        "SimSun"
                    } else {
                        &style.family
                    };
                    let label = xml(&text[node.range()]);
                    let svg = format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" style=\"width:{}em;height:{}em;vertical-align:middle\"><title>{label}</title><g fill=\"{}\" color=\"{}\" font-family=\"{}\">{}</g></svg>",
                        layout.w,
                        layout.h,
                        layout.w,
                        layout.h,
                        layout.w / 32.0,
                        layout.h / 32.0,
                        style.foreground,
                        style.foreground,
                        xml(family),
                        layout.svg
                    );
                    replacements
                        .push((node.range(), svg.replace("currentColor", &style.foreground)));
                    if tag == "math" {
                        math_count += 1;
                    } else {
                        ruby_count += 1;
                    }
                } else {
                    unsupported += 1;
                }
            }
            for (range, svg) in replacements.into_iter().rev() {
                text.replace_range(range, &svg);
            }
            target.write_all(text.as_bytes())?;
        } else {
            // Preserve unchanged compressed payloads; no decode/recompress or
            // expansion of a large book just to repair a few inline elements.
            target.raw_copy_file(item)?;
        }
    }
    target.finish()?;
    result.notice =
        format!("原生矢量兼容：Ruby {ruby_count} 处、MathML {math_count} 处（保留矢量文字）");
    if unsupported > 0 {
        result.notice.push_str(&format!(
            " · {unsupported} 处复杂结构未转换，原生显示可能有差异"
        ));
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fraction_and_ruby_keep_two_levels_and_unsupported_math_is_not_removed() {
        let doc = roxmltree::Document::parse(
            "<math><mfrac><mi>a</mi><mi>b</mi></mfrac><msup><mi>x</mi><mn>2</mn></msup></math>",
        )
        .unwrap();
        let layout = math(doc.root_element(), 0).unwrap();
        assert!(layout.h > 60.0 && layout.svg.contains("<path"));
        let doc = roxmltree::Document::parse("<ruby>漢<rt>かん</rt>字<rt>じ</rt></ruby>").unwrap();
        let layout = ruby(doc.root_element()).unwrap();
        assert!(layout.svg.contains("かん") && layout.svg.contains("漢"));
        assert!(layout.h > 40.0);
        let doc = roxmltree::Document::parse("<math><maction><mi>x</mi></maction></math>").unwrap();
        assert!(math(doc.root_element(), 0).is_none());
    }
}
