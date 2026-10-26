use crate::{security, settings::Typography};
use anyhow::{Context, Result, bail, ensure};
use mupdf::{Document, MetadataName, Outline};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Book {
    pub engine: Document,
    pub path: PathBuf,
    pub title: String,
    pub reflowable: bool,
    pub page_count: i32,
    pub layout_revision: u64,
    pub pagination_complete: bool,
    pub notice: String,
    pub encoding: Option<String>,
    pub conversion_style: Option<(String, String)>,
    _converted: Option<crate::text_adapter::Converted>,
    style: Option<Typography>,
    viewport: (i32, i32),
    chapters: Vec<Option<i32>>,
    next_chapter: usize,
    contents: Option<Arc<Vec<ContentsItem>>>,
}

#[derive(Clone, Debug)]
pub struct ContentsItem {
    pub title: String,
    pub uri: String,
    pub depth: usize,
}

impl Book {
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_encoded(path, None)
    }
    pub fn open_encoded(path: &Path, encoding: Option<&str>) -> Result<Self> {
        Self::open_styled(path, encoding, &Typography::default())
    }
    pub fn open_styled(
        path: &Path,
        encoding: Option<&str>,
        typography: &Typography,
    ) -> Result<Self> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        ensure!(
            path.metadata()
                .context("无法读取文件，请检查文件是否已移动或被锁定")?
                .len()
                <= security::MAX_FILE,
            "文件超过当前 512 MiB 安全上限"
        );
        let mut notice = String::new();
        let mut converted = None;
        let mut fixed_viewport = None;
        let mut conversion_style = None;
        match extension.as_str() {
            "epub" => {
                let report = security::inspect_epub(path)?;
                converted = crate::epub_compat::convert(path, &report, typography)?;
                if let Some(compat) = &converted {
                    notice = compat.notice.clone();
                    conversion_style =
                        Some((typography.foreground.clone(), typography.family.clone()));
                }
                if report.repaired_html > 0 {
                    if !notice.is_empty() {
                        notice.push_str(" · ");
                    }
                    notice.push_str(&format!(
                        "书内 {} 个页面含不规范标记，已启用 HTML 兼容读取",
                        report.repaired_html
                    ));
                }
                if report.fixed {
                    fixed_viewport = report.viewport;
                    if fixed_viewport.is_none() {
                        notice.push_str(" · 固定布局缺少有效 viewport，暂按可重排版式打开");
                    }
                }
                if report.vertical {
                    notice.push_str(" · 此书含竖排；当前原生引擎尚不能保证竖排方向与标点位置");
                }
                if report.unconverted_complex_html {
                    notice.push_str(" · 非规范 HTML 中的注音或公式尚未转换，原生显示可能有差异");
                }
            }
            "pdf" => {}
            "txt" | "md" => {
                let document = crate::text_adapter::convert(path, encoding)?;
                notice = document.notice.clone();
                converted = Some(document);
            }
            _ => bail!("仅支持 EPUB、PDF、TXT 和 Markdown 四种格式"),
        }
        let native_path = converted
            .as_ref()
            .map(|c| c.path.as_path())
            .unwrap_or(path)
            .to_str()
            .context("文件路径包含无法表示的 Unicode，请先重命名文件")?;
        let mut engine = Document::open(native_path).context("原生引擎无法打开此文件")?;
        ensure!(
            !engine.needs_password()?,
            "此文档需要密码；当前验证版尚无密码输入界面"
        );
        let reflowable = engine.is_reflowable()? && fixed_viewport.is_none();
        if let Some((width, height)) = fixed_viewport {
            engine.set_style("@page{margin:0!important}", true)?;
            engine.set_fixed_layout()?;
            engine.layout(width, height, 16.0)?;
            notice.push_str(" · 固定布局按书内画布尺寸显示");
        }
        notice = notice.trim_start_matches([' ', '·']).to_owned();
        let mut title = engine.metadata(MetadataName::Title).unwrap_or_default();
        if title.is_empty() {
            title = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
        }
        Ok(Self {
            engine,
            path: path.to_owned(),
            title,
            reflowable,
            page_count: 0,
            layout_revision: 0,
            pagination_complete: false,
            notice,
            encoding: converted
                .as_ref()
                .filter(|c| !c.encoding.is_empty())
                .map(|c| c.encoding.clone()),
            conversion_style,
            _converted: converted,
            style: None,
            viewport: (0, 0),
            chapters: Vec::new(),
            next_chapter: 0,
            contents: None,
        })
    }

    pub fn layout(
        &mut self,
        style: &Typography,
        width_pt: f32,
        height_pt: f32,
        current: i32,
    ) -> Result<i32> {
        let dimensions = (width_pt.floor() as i32, height_pt.floor() as i32);
        if !self.reflowable && self.page_count > 0 {
            return Ok(current);
        }
        if self.style.as_ref() == Some(style) && self.viewport == dimensions {
            return Ok(current);
        }
        let anchor = if self.page_count > 0 {
            self.engine
                .logical_anchor(current.clamp(0, self.page_count - 1))
                .ok()
        } else {
            None
        };
        if self.reflowable {
            if self.style.as_ref() != Some(style) {
                self.engine.set_style(&style.css(), true)?;
            }
            self.engine.layout(
                dimensions.0.max(120) as f32,
                dimensions.1.max(120) as f32,
                style.size_pt,
            )?;
        }
        self.chapters = if self.reflowable {
            vec![None; self.engine.chapter_count()?.max(1) as usize]
        } else {
            vec![Some(self.engine.page_count()?)]
        };
        self.page_count = self.chapters.iter().flatten().sum();
        self.next_chapter = if self.reflowable {
            0
        } else {
            self.chapters.len()
        };
        self.pagination_complete = self.next_chapter == self.chapters.len();
        self.style = Some(style.clone());
        self.viewport = dimensions;
        self.layout_revision += 1;
        let restored = anchor
            .and_then(|a| self.engine.resolve_anchor(a).ok().flatten())
            .unwrap_or(current)
            .max(0);
        self.ensure_page(restored)?;
        ensure!(self.page_count > 0, "文档没有可读取页面");
        Ok(restored.min(self.page_count - 1))
    }

    pub fn ensure_page(&mut self, page: i32) -> Result<()> {
        while self.page_count <= page && self.next_chapter < self.chapters.len() {
            self.count_chapter(self.next_chapter)?;
        }
        Ok(())
    }
    fn count_chapter(&mut self, index: usize) -> Result<()> {
        if self.chapters[index].is_none() {
            let count = self.engine.chapter_page_count(index as i32)?.max(0);
            self.chapters[index] = Some(count);
            self.page_count += count;
        }
        while self.next_chapter < self.chapters.len() && self.chapters[self.next_chapter].is_some()
        {
            self.next_chapter += 1;
        }
        self.pagination_complete = self.next_chapter == self.chapters.len();
        Ok(())
    }
    pub fn paginate_next(&mut self) -> Result<bool> {
        if self.next_chapter < self.chapters.len() {
            self.count_chapter(self.next_chapter)?;
            Ok(true)
        } else {
            self.pagination_complete = true;
            Ok(false)
        }
    }

    pub fn contents(&mut self) -> Result<Arc<Vec<ContentsItem>>> {
        if let Some(cached) = &self.contents {
            return Ok(cached.clone());
        }
        fn flatten(nodes: Vec<Outline>, depth: usize, out: &mut Vec<ContentsItem>) {
            if depth > 32 {
                return;
            }
            for node in nodes {
                if out.len() >= 5000 {
                    return;
                }
                if let Some(uri) = node.uri {
                    out.push(ContentsItem {
                        title: node.title,
                        uri,
                        depth,
                    });
                }
                flatten(node.down, depth + 1, out);
            }
        }
        let mut rows = Vec::new();
        flatten(self.engine.outlines_unresolved()?, 0, &mut rows);
        let rows = Arc::new(rows);
        self.contents = Some(rows.clone());
        Ok(rows)
    }
}
