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
    style: Option<Typography>,
    viewport: (i32, i32),
    chapters: Vec<Option<i32>>,
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
        match extension.as_str() {
            "epub" => {
                let report = security::inspect_epub(path)?;
                if report.repaired_html > 0 {
                    notice = format!(
                        "书内 {} 个页面含不规范标记，已启用 HTML 兼容读取",
                        report.repaired_html
                    );
                }
            }
            "pdf" => {}
            "txt" | "md" => {
                bail!("TXT / Markdown 适配器尚未通过 G2，目前请使用 EPUB / PDF 技术验证")
            }
            _ => bail!("仅支持 EPUB、PDF、TXT 和 Markdown 四种格式"),
        }
        let native_path = path
            .to_str()
            .context("文件路径包含无法表示的 Unicode，请先重命名文件")?;
        let engine = Document::open(native_path).context("原生引擎无法打开此文件")?;
        ensure!(
            !engine.needs_password()?,
            "此文档需要密码；当前验证版尚无密码输入界面"
        );
        let reflowable = engine.is_reflowable()?;
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
            style: None,
            viewport: (0, 0),
            chapters: Vec::new(),
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
        self.page_count = 0;
        self.pagination_complete = false;
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
        for i in 0..self.chapters.len() {
            self.count_chapter(i)?;
            if self.page_count > page {
                break;
            }
        }
        Ok(())
    }
    fn count_chapter(&mut self, index: usize) -> Result<()> {
        if self.chapters[index].is_none() {
            self.chapters[index] = Some(self.engine.chapter_page_count(index as i32)?);
        }
        self.page_count = self.chapters.iter().filter_map(|p| *p).sum();
        self.pagination_complete = self.chapters.iter().all(Option::is_some);
        Ok(())
    }
    pub fn paginate_next(&mut self) -> Result<bool> {
        if let Some(i) = self.chapters.iter().position(Option::is_none) {
            self.count_chapter(i)?;
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
