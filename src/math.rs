//! On-demand TeX -> MathML -> native SVG text. No web runtime or bundled font.
//! All parser/layout work is temporary; the document retains only its SVG nodes.
use anyhow::{Context, Result, bail, ensure};
use mupdf::Font;
use roxmltree::Node;

const EM: f32 = 32.0;
const MAX_TEX: usize = 8192;
const MAX_EVENTS: usize = 4096;

#[derive(Default)]
pub struct MarkdownMath {
    typesetter: Option<Option<Typesetter>>,
    pub rendered: usize,
    pub failed: usize,
    bytes: usize,
}
impl MarkdownMath {
    pub fn html(&mut self, tex: &str, display: bool, color: &str) -> String {
        let result = (|| -> Result<String> {
            ensure!(
                self.rendered + self.failed < 1024 && self.bytes < 8 * 1024 * 1024,
                "文档公式超过上限"
            );
            let html = self
                .typesetter
                .get_or_insert_with(|| Typesetter::new().ok())
                .as_ref()
                .context("系统数学字体不可用")?
                .render(tex, display, color)?;
            ensure!(
                self.bytes + html.len() <= 8 * 1024 * 1024,
                "文档公式超过上限"
            );
            Ok(html)
        })();
        match result {
            Ok(html) => {
                self.rendered += 1;
                self.bytes += html.len();
                html
            }
            Err(_) => {
                self.failed += 1;
                if display {
                    format!("<pre>$$\n{}\n$$</pre>", xml(tex))
                } else {
                    format!("<code>${}$</code>", xml(tex))
                }
            }
        }
    }
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub struct Typesetter {
    regular: Font,
    italic: Font,
}

struct Boxed {
    w: f32,
    h: f32,
    base: f32,
    svg: String,
}
impl Boxed {
    fn at(&self, x: f32, y: f32, scale: f32) -> String {
        format!(
            "<g transform=\"translate({x} {y}) scale({scale})\">{}</g>",
            self.svg
        )
    }
    fn scaled(mut self, scale: f32) -> Self {
        self.svg = self.at(0.0, 0.0, scale);
        self.w *= scale;
        self.h *= scale;
        self.base *= scale;
        self
    }
}

fn group(children: Vec<Boxed>) -> Boxed {
    let base = children.iter().map(|b| b.base).fold(0.0, f32::max);
    let below = children.iter().map(|b| b.h - b.base).fold(0.0, f32::max);
    let mut x = 0.0;
    let mut svg = String::new();
    for child in children {
        svg.push_str(&child.at(x, base - child.base, 1.0));
        x += child.w;
    }
    Boxed {
        w: x,
        h: base + below,
        base,
        svg,
    }
}

impl Typesetter {
    pub fn new() -> Result<Self> {
        Ok(Self {
            regular: crate::fonts::math_font().context("系统数学字体不可用")?,
            italic: crate::fonts::math_italic().context("系统数学斜体不可用")?,
        })
    }
    fn leaf(&self, value: &str, italic: bool) -> Result<Boxed> {
        let font = if italic { &self.italic } else { &self.regular };
        let mut w = 0.0;
        for c in value.chars() {
            let glyph = font.encode_character(c as i32)?;
            // Let the same Windows font loader provide CJK for \text{...}.
            w += if glyph != 0 {
                font.advance_glyph(glyph)? * EM
            } else if matches!(c as u32, 0x2e80..=0x9fff | 0xac00..=0xd7ff | 0xf900..=0xfaff | 0x20000..=0x3134f)
            {
                let fallback = crate::fonts::math_cjk().context("数学文字回退字体不可用")?;
                let glyph = fallback.encode_character(c as i32)?;
                ensure!(glyph != 0, "数学字形不可用");
                fallback.advance_glyph(glyph)? * EM
            } else {
                bail!("数学字形不可用")
            };
        }
        Ok(Boxed {
            w: w + if italic { 2.0 } else { 0.0 },
            h: 40.0,
            base: 30.0,
            svg: format!(
                "<text x=\"0\" y=\"30\" font-family=\"{}\" font-size=\"32\"{}>{}</text>",
                if italic {
                    "Times New Roman"
                } else {
                    "Cambria Math"
                },
                if italic { " font-style=\"italic\"" } else { "" },
                xml(value)
            ),
        })
    }
    fn layout(&self, node: Node<'_, '_>, depth: usize, display: bool) -> Result<Boxed> {
        ensure!(depth <= 32, "公式嵌套超过上限");
        ensure!(
            node.attribute("class") != Some("mop-negated"),
            "暂不支持组合否定符号"
        );
        ensure!(
            !node
                .attribute("style")
                .is_some_and(|s| s.contains("color") || s.contains("font-size")),
            "暂不支持公式局部颜色或字号命令"
        );
        let display = node
            .attribute("displaystyle")
            .map_or(display, |v| v == "true");
        let tag = node.tag_name().name();
        if matches!(tag, "mi" | "mn" | "mo" | "mtext" | "ms") {
            let value: String = node
                .descendants()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect();
            let italic = tag == "mi"
                && value.chars().count() == 1
                && node.attribute("mathvariant") != Some("normal")
                && value
                    .chars()
                    .all(|c| c.is_ascii_alphabetic() || ('\u{03b1}'..='\u{03c9}').contains(&c));
            let mut leaf = self.leaf(&value, italic)?;
            if tag == "mo" {
                if display
                    && (node.attribute("largeop") == Some("true")
                        || (node.attribute("largeop") != Some("false")
                            && matches!(value.as_str(), "∑" | "∏" | "∫" | "∬" | "∭" | "⋃" | "⋂")))
                {
                    leaf = leaf.scaled(1.35);
                }
                let gap = if matches!(
                    value.as_str(),
                    "=" | "+" | "−" | "<" | ">" | "≤" | "≥" | "≈" | "∈" | "→" | "⇒"
                ) {
                    5.0
                } else if matches!(value.as_str(), "," | ";") {
                    2.0
                } else {
                    0.0
                };
                leaf.svg = leaf.at(gap, 0.0, 1.0);
                leaf.w += gap * 2.0;
            }
            return Ok(leaf);
        }
        if tag == "mspace" {
            let value = node.attribute("width").unwrap_or("0em");
            let w = value
                .strip_suffix("em")
                .and_then(|n| n.parse::<f32>().ok())
                .unwrap_or(0.0)
                * EM;
            ensure!(
                w.is_finite() && w >= 0.0 && w <= 8.0 * EM,
                "暂不支持负间距或过长的公式空白"
            );
            return Ok(Boxed {
                w,
                h: 0.0,
                base: 0.0,
                svg: String::new(),
            });
        }
        let nodes = node.children().filter(Node::is_element).collect::<Vec<_>>();
        if tag == "mtable" {
            return self.table(node, depth, display);
        }
        let mut children = nodes
            .iter()
            .map(|n| self.layout(*n, depth + 1, display))
            .collect::<Result<Vec<_>>>()?;
        match tag {
            "math" | "mrow" | "mstyle" | "mtd" | "semantics" => {
                let height = children.iter().map(|b| b.h).fold(40.0, f32::max);
                // Stretch explicit delimiters to their enclosed expression.
                if height > 48.0 {
                    for (n, b) in nodes.iter().zip(&mut children) {
                        if n.tag_name().name() == "mo"
                            && n.attribute("stretchy") == Some("true")
                            && matches!(
                                n.text().unwrap_or(""),
                                "(" | ")" | "[" | "]" | "{" | "}" | "|" | "‖"
                            )
                        {
                            b.svg =
                                format!("<g transform=\"scale(1 {})\">{}</g>", height / b.h, b.svg);
                            b.base = height / 2.0 + 8.0;
                            b.h = height;
                        }
                    }
                }
                Ok(group(children))
            }
            "mfrac" if children.len() == 2 => {
                let a = &children[0];
                let b = &children[1];
                let scale = if display { 0.95 } else { 0.8 };
                let w = a.w.max(b.w) * scale + 10.0;
                let rule = a.h * scale + 3.0;
                let bottom = rule + 7.0;
                let line = if node.attribute("linethickness") == Some("0")
                    || node.attribute("linethickness") == Some("0px")
                {
                    String::new()
                } else {
                    format!(
                        "<path d=\"M1 {rule} H{}\" stroke=\"currentColor\" stroke-width=\"1.4\"/>",
                        w - 1.0
                    )
                };
                Ok(Boxed {
                    w,
                    h: bottom + b.h * scale,
                    base: rule + 8.0,
                    svg: format!(
                        "{}{line}{}",
                        a.at((w - a.w * scale) / 2.0, 0.0, scale),
                        b.at((w - b.w * scale) / 2.0, bottom, scale)
                    ),
                })
            }
            "msup" | "msub" | "msubsup" if children.len() >= 2 => {
                let a = &children[0];
                let sub = if tag != "msup" { children.get(1) } else { None };
                let sup = if tag == "msup" {
                    children.get(1)
                } else if tag == "msubsup" {
                    children.get(2)
                } else {
                    None
                };
                let scale = 0.68;
                let sy = sup.map_or(0.0, |b| a.base - 16.0 - b.base * scale);
                let shift = (-sy).max(0.0);
                let mut svg = a.at(0.0, shift, 1.0);
                let mut h = a.h + shift;
                let mut w = 0.0f32;
                if let Some(b) = sup {
                    svg.push_str(&b.at(a.w + 1.0, sy + shift, scale));
                    w = b.w * scale;
                    h = h.max(sy + shift + b.h * scale);
                }
                if let Some(b) = sub {
                    let y = a.base + 8.0 - b.base * scale + shift;
                    svg.push_str(&b.at(a.w + 1.0, y, scale));
                    w = w.max(b.w * scale);
                    h = h.max(y + b.h * scale);
                }
                Ok(Boxed {
                    w: a.w + 1.0 + w,
                    h,
                    base: a.base + shift,
                    svg,
                })
            }
            "msqrt" | "mroot" => {
                let (a, index) = if tag == "mroot" {
                    ensure!(children.len() == 2, "根式结构无效");
                    let index = children.pop().unwrap();
                    (children.pop().unwrap(), Some(index))
                } else {
                    (group(children), None)
                };
                let left = index
                    .as_ref()
                    .map_or(20.0, |b| (b.w * 0.5 + 10.0).max(20.0));
                let h = a.h + 6.0;
                let w = a.w + left + 3.0;
                let mut svg = format!(
                    "<path d=\"M{} {} L{} {} L{} {} L{} 2 H{w}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\"/>{}",
                    left - 19.0,
                    h * 0.55,
                    left - 14.0,
                    h * 0.5,
                    left - 9.0,
                    h - 3.0,
                    left - 1.0,
                    a.at(left + 2.0, 6.0, 1.0)
                );
                if let Some(b) = index {
                    svg.push_str(&b.at(0.0, (h * 0.5 - b.h * 0.5).max(0.0), 0.5));
                }
                Ok(Boxed {
                    w,
                    h,
                    base: a.base + 6.0,
                    svg,
                })
            }
            "munder" | "mover" | "munderover" if children.len() >= 2 => {
                let a = &children[0];
                let under = if tag != "mover" {
                    children.get(1)
                } else {
                    None
                };
                let over = if tag == "mover" {
                    children.get(1)
                } else if tag == "munderover" {
                    children.get(2)
                } else {
                    None
                };
                let scale = 0.68;
                let w =
                    a.w.max(under.map_or(0.0, |b| b.w * scale))
                        .max(over.map_or(0.0, |b| b.w * scale));
                let top = over.map_or(0.0, |b| b.h * scale + 3.0);
                let mut svg = a.at((w - a.w) / 2.0, top, 1.0);
                if let Some(b) = over {
                    svg.push_str(&b.at((w - b.w * scale) / 2.0, 0.0, scale));
                }
                if let Some(b) = under {
                    svg.push_str(&b.at((w - b.w * scale) / 2.0, top + a.h + 3.0, scale));
                }
                Ok(Boxed {
                    w,
                    h: top + a.h + under.map_or(0.0, |b| b.h * scale + 3.0),
                    base: top + a.base,
                    svg,
                })
            }
            _ => bail!("暂不支持的公式结构"),
        }
    }
    fn table(&self, node: Node<'_, '_>, depth: usize, display: bool) -> Result<Boxed> {
        let mut rows = Vec::new();
        let mut widths = Vec::<f32>::new();
        for row in node.children().filter(Node::is_element) {
            ensure!(row.tag_name().name() == "mtr", "暂不支持的公式表格");
            let mut cells = Vec::new();
            for (i, cell) in row.children().filter(Node::is_element).enumerate() {
                ensure!(i < 16 && cell.tag_name().name() == "mtd", "公式列超过上限");
                let b = self.layout(cell, depth + 2, display)?;
                if i == widths.len() {
                    widths.push(b.w)
                } else {
                    widths[i] = widths[i].max(b.w)
                }
                cells.push(b);
            }
            rows.push(cells);
            ensure!(rows.len() <= 64, "公式行超过上限");
        }
        let classes = node.attribute("class").unwrap_or("");
        ensure!(
            !classes.contains("border")
                && !node.descendants().any(|n| n
                    .attribute("class")
                    .is_some_and(|c| c.contains("border")
                        || c.contains("hline")
                        || c.contains("hdashline"))),
            "暂不支持带线公式表格"
        );
        let aligned = classes.contains("alignlike");
        let default_align = if classes.contains("cells-left") {
            "left"
        } else if classes.contains("cells-right") {
            "right"
        } else {
            "center"
        };
        let align = node
            .attribute("columnalign")
            .unwrap_or(default_align)
            .split_whitespace()
            .collect::<Vec<_>>();
        let gap = |i: usize| if aligned && i % 2 == 0 { 2.0 } else { 16.0 };
        let mut y = 0.0;
        let mut svg = String::new();
        for cells in rows {
            let base = cells.iter().map(|b| b.base).fold(0.0, f32::max);
            let below = cells.iter().map(|b| b.h - b.base).fold(0.0, f32::max);
            let mut x = 0.0;
            for (i, b) in cells.iter().enumerate() {
                let alignment = if aligned {
                    if i % 2 == 0 { "right" } else { "left" }
                } else {
                    align
                        .get(i)
                        .or_else(|| align.last())
                        .copied()
                        .unwrap_or("center")
                };
                let offset = match alignment {
                    "left" => 0.0,
                    "right" => widths[i] - b.w,
                    _ => (widths[i] - b.w) / 2.0,
                };
                svg.push_str(&b.at(x + offset, y + base - b.base, 1.0));
                x += widths[i] + gap(i);
            }
            y += base + below + 8.0;
        }
        let h = (y - 8.0).max(0.0);
        Ok(Boxed {
            w: widths.iter().sum::<f32>()
                + (0..widths.len().saturating_sub(1)).map(gap).sum::<f32>(),
            h,
            base: h / 2.0 + 8.0,
            svg,
        })
    }
    pub fn render(&self, tex: &str, display: bool, color: &str) -> Result<String> {
        ensure!(tex.len() <= MAX_TEX, "公式过长");
        let mut depth = 0usize;
        for c in tex.chars() {
            if c == '{' {
                depth += 1;
                ensure!(depth <= 32, "公式嵌套超过上限")
            } else if c == '}' {
                depth = depth.saturating_sub(1)
            }
        }
        // User-defined macros can amplify a tiny input. Standard math commands
        // remain available; no filesystem/network commands are ever executed.
        for forbidden in [
            "\\def",
            "\\gdef",
            "\\edef",
            "\\xdef",
            "\\let",
            "\\newcommand",
            "\\renewcommand",
            "\\csname",
        ] {
            ensure!(!tex.contains(forbidden), "暂不支持自定义公式宏");
        }
        let storage = pulldown_latex::Storage::new();
        let mut count = 0;
        let events = pulldown_latex::Parser::new(tex, &storage).map(|event| {
            count += 1;
            if count > MAX_EVENTS {
                return Err(std::io::Error::other("公式结构超过上限"));
            }
            event.map_err(|_| std::io::Error::other("公式语法无法解析"))
        });
        let mut mathml = String::new();
        pulldown_latex::push_mathml(
            &mut mathml,
            events,
            pulldown_latex::RenderConfig {
                xml: true,
                display_mode: if display {
                    pulldown_latex::config::DisplayMode::Block
                } else {
                    pulldown_latex::config::DisplayMode::Inline
                },
                ..Default::default()
            },
        )?;
        ensure!(
            mathml.len() <= 128 * 1024 && !mathml.contains("<merror"),
            "公式无法完整解析"
        );
        let doc = roxmltree::Document::parse_with_options(
            &mathml,
            roxmltree::ParsingOptions {
                nodes_limit: 8192,
                ..Default::default()
            },
        )?;
        let b = self.layout(doc.root_element(), 0, display)?;
        ensure!(
            b.w.is_finite()
                && b.h.is_finite()
                && b.w >= 0.0
                && b.h >= 0.0
                && b.w <= 20000.0
                && b.h <= 10000.0
                && b.svg.len() <= 256 * 1024,
            "公式排版超过上限"
        );
        // Extra edge padding retains italic overhang and anti-aliased rules.
        let w = b.w + 12.0;
        let axis = b.base - EM * 0.25;
        let half = (axis + 2.0).max(b.h - axis + 2.0);
        let h = half * 2.0;
        Ok(format!(
            "<span class=\"sae-math-{}\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" style=\"width:{}em;height:{}em;vertical-align:middle\"><title>{}</title><g fill=\"{color}\" color=\"{color}\">{}</g></svg></span>",
            if display { "display" } else { "inline" },
            w / EM,
            h / EM,
            xml(tex),
            b.at(6.0, half - axis, 1.0).replace("currentColor", color)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_tex_supports_common_structures_and_rejects_partial_or_unbounded_math() {
        crate::fonts::install();
        let math = Typesetter::new().unwrap();
        for tex in [
            r"I_C=C\frac{du}{dt}",
            r"\tau\frac{du}{dt}=-(u-u_{\mathrm{rest}})+RI(t)",
            r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
            r"\sum_{i=1}^{n} i^2",
            r"\int_0^1 x^2\,dx",
            r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
            r"\begin{aligned}I&=I_C+I_R\\R I&=RC\frac{du}{dt}+u-u_{rest}\end{aligned}",
            r"\sqrt[3]{x}",
            r"\text{中文}",
        ] {
            math.render(tex, true, "#262A27")
                .unwrap_or_else(|e| panic!("{tex}: {e:#}"));
        }
        assert!(
            math.render(r"\unknowncommand{x}", false, "#262A27")
                .is_err()
        );
        assert!(math.render(r"\def\a{\a}\a", false, "#262A27").is_err());
        assert!(math.render(&"{".repeat(100), false, "#262A27").is_err());
        assert!(math.render(r"\!x", false, "#262A27").is_err());
    }
}
