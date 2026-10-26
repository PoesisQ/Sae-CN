//! Windows font fallback without bundling Windows fonts or scanning them per glyph.
use mupdf::{CjkFontOrdering, Font, FontHints, FontLoader};
use std::{cell::RefCell, collections::VecDeque, path::PathBuf};

const FONT_BUDGET: usize = 32 * 1024 * 1024;
thread_local! {
    static FONTS: RefCell<VecDeque<(String, Font, usize)>> = const { RefCell::new(VecDeque::new()) };
}

pub fn install() {
    mupdf::set_font_loader(WindowsFonts);
}
pub fn clear_thread_cache() {
    FONTS.with(|fonts| fonts.borrow_mut().clear());
}

struct WindowsFonts;

fn load(file: &str, index: i32) -> Option<Font> {
    let key = format!("{file}:{index}");
    if let Some(found) = FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        let position = fonts.iter().position(|(k, _, _)| *k == key)?;
        let item = fonts.remove(position)?;
        let font = item.1.clone();
        fonts.push_back(item);
        Some(font)
    }) {
        return Some(found);
    }
    let path = PathBuf::from(std::env::var_os("WINDIR")?)
        .join("Fonts")
        .join(file);
    let size = std::fs::metadata(&path).ok()?.len() as usize;
    if size > FONT_BUDGET {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let font = Font::from_bytes_with_index(file, index, &bytes).ok()?;
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        while fonts.iter().map(|(_, _, s)| *s).sum::<usize>() + size > FONT_BUDGET {
            if fonts.pop_front().is_none() {
                break;
            }
        }
        fonts.push_back((key, font.clone(), size));
    });
    Some(font)
}

impl FontLoader for WindowsFonts {
    fn load_font(&self, name: &str, hints: FontHints) -> Option<Font> {
        let key = name.to_lowercase();
        let file = match key.as_str() {
            "simsun" | "宋体" => "simsun.ttc",
            "microsoft yahei" | "微软雅黑" => {
                if hints.bold {
                    "msyhbd.ttc"
                } else {
                    "msyh.ttc"
                }
            }
            "segoe ui" => match (hints.bold, hints.italic) {
                (true, true) => "segoeuiz.ttf",
                (true, false) => "segoeuib.ttf",
                (false, true) => "segoeuii.ttf",
                _ => "segoeui.ttf",
            },
            "times new roman" => match (hints.bold, hints.italic) {
                (true, true) => "timesbi.ttf",
                (true, false) => "timesbd.ttf",
                (false, true) => "timesi.ttf",
                _ => "times.ttf",
            },
            "consolas" => match (hints.bold, hints.italic) {
                (true, true) => "consolaz.ttf",
                (true, false) => "consolab.ttf",
                (false, true) => "consolai.ttf",
                _ => "consola.ttf",
            },
            _ => return None,
        };
        let font = load(file, 0)?;
        if hints.needs_exact_metrics
            && ((hints.bold && !font.is_bold()) || (hints.italic && !font.is_italic()))
        {
            return None;
        }
        Some(font)
    }

    fn load_cjk_font(&self, _name: &str, ordering: CjkFontOrdering, serif: bool) -> Option<Font> {
        let preferred = match ordering {
            CjkFontOrdering::AdobeJapan => "msmincho.ttc",
            CjkFontOrdering::AdobeKorea => "malgun.ttf",
            _ if serif => "simsun.ttc",
            _ => "msyh.ttc",
        };
        load(preferred, 0).or_else(|| load("simsun.ttc", 0))
    }

    fn load_fallback_font(&self, script: u32, _language: u32, hints: FontHints) -> Option<Font> {
        // UCDN values verified in the pinned MuPDF ucdn.h, not guessed names.
        match script {
            24 => load("malgun.ttf", 0).or_else(|| load("simsun.ttc", 0)),
            32 | 33 => load("msmincho.ttc", 0).or_else(|| load("simsun.ttc", 0)),
            34 | 35 => load(
                if hints.serif {
                    "simsun.ttc"
                } else {
                    "msyh.ttc"
                },
                0,
            ),
            5 | 6 => load("times.ttf", 0),
            _ => load("segoeui.ttf", 0),
        }
    }
}
