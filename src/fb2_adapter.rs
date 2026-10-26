//! MuPDF renders FB2 directly. Rust adds stable navigation IDs only when needed;
//! no content/style/image conversion and no retained second XML tree.
use crate::{document::ContentsItem, text_adapter::Converted};
use anyhow::{Context, Result, ensure};
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

pub fn style_css(style: &crate::settings::Typography) -> String {
    use std::fmt::Write;
    let mut css = style.css();
    // MuPDF's FB2 handler does not promote body background to the page root.
    // Paint its real canvas via @page; do not recolor pixels or illustrations.
    write!(
        css,
        "\n@page {{ background-color: {}; }}\n",
        style.background
    )
    .unwrap();
    css
}

pub fn prepare(path: &Path) -> Result<(Option<Converted>, Arc<Vec<ContentsItem>>)> {
    let file = File::open(path)?;
    ensure!(
        file.metadata()?.len() <= 32 * 1024 * 1024,
        "FB2 超过当前 32 MiB 上限"
    );
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 32 * 1024 * 1024, "FB2 超过当前 32 MiB 上限");
    let (encoding, skip) = if let Some(bom) = encoding_rs::Encoding::for_bom(&bytes) {
        bom
    } else {
        let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(160)]).to_ascii_lowercase();
        let declaration = prefix
            .strip_prefix("<?xml")
            .and_then(|s| s.split_once("?>"))
            .map_or("", |(s, _)| s);
        let label = declaration
            .split_once("encoding")
            .and_then(|(_, s)| s.split_once('='))
            .map(|(_, s)| s.trim_start().trim_start_matches(['\'', '"']))
            .and_then(|s| s.split(['\'', '"']).next());
        let encoding = match label {
            Some(label) => {
                encoding_rs::Encoding::for_label(label.as_bytes()).context("FB2 编码不受支持")?
            }
            None => encoding_rs::UTF_8,
        };
        (encoding, 0)
    };
    let (text, errors) = encoding.decode_without_bom_handling(&bytes[skip..]);
    ensure!(
        !errors && text.len() <= 32 * 1024 * 1024,
        "FB2 编码损坏或解码后超过上限"
    );
    ensure!(
        !text.match_indices("<!").any(|(at, _)| text
            .as_bytes()
            .get(at..at + 8)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"<!ENTITY"))),
        "FB2 不允许 XML 实体声明"
    );
    let doc = roxmltree::Document::parse_with_options(
        &text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 300_000,
        },
    )
    .context("FB2 XML 损坏")?;
    ensure!(
        doc.root_element().tag_name().name() == "FictionBook",
        "不是有效的 FictionBook 文档"
    );
    let mut used = HashSet::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        ensure!(
            node.ancestors().take(129).count() <= 128,
            "FB2 嵌套超过上限"
        );
        ensure!(
            node.tag_name().name() != "script",
            "FB2 脚本不受支持，不执行脚本"
        );
        if let Some(id) = node.attribute("id") {
            ensure!(
                id.len() <= 512 && used.len() < 10_000,
                "FB2 ID 长度或数量超过上限"
            );
            ensure!(used.insert(id.to_owned()), "FB2 ID 重复");
        }
        if node.tag_name().name() == "image" {
            for a in node.attributes().filter(|a| a.name() == "href") {
                ensure!(a.value().starts_with('#'), "FB2 仅允许书内嵌入图片");
            }
        }
    }
    let mut contents = Vec::new();
    let mut insertions = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("section")) {
        let Some(title) = node.children().find(|n| n.has_tag_name("title")) else {
            continue;
        };
        ensure!(contents.len() < 5000, "FB2 章节数量超过上限");
        let title: String = title
            .descendants()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .flat_map(str::chars)
            .take(512)
            .collect();
        let id = if let Some(id) = node.attribute("id") {
            id.to_owned()
        } else {
            let mut n = contents.len();
            let id = loop {
                let id = format!("_sae_section_{n}");
                if used.insert(id.clone()) {
                    break id;
                }
                n += 1;
            };
            let start = node.range().start + 1;
            let end = text[start..]
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .context("FB2 章节标记损坏")?
                + start;
            insertions.push((end, format!(" id=\"{id}\"")));
            id
        };
        contents.push(ContentsItem {
            title: title.trim().to_owned(),
            uri: format!("#{id}"),
            depth: node
                .ancestors()
                .filter(|n| n.has_tag_name("section"))
                .count()
                .saturating_sub(1),
        });
    }
    let converted = if !insertions.is_empty() || encoding != encoding_rs::UTF_8 {
        let mut output = Converted::temporary("fb2")?;
        output.path.set_extension("fb2");
        let mut file = File::create(&output.path)?;
        file.write_all(b"<?xml version=\"1.0\" encoding=\"utf-8\"?>")?;
        let mut cursor = if text.starts_with("<?xml") {
            text.find("?>").context("FB2 XML 声明损坏")? + 2
        } else {
            0
        };
        for (at, addition) in insertions {
            file.write_all(text[cursor..at].as_bytes())?;
            file.write_all(addition.as_bytes())?;
            cursor = at;
        }
        file.write_all(text[cursor..].as_bytes())?;
        Some(output)
    } else {
        None
    };
    Ok((converted, Arc::new(contents)))
}
