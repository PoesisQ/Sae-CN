#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use sae_reader::{
    paging::WheelPager,
    reading::{Mailbox, Navigation, Request, Worker},
    rendering::{Bounds, Frame},
    settings::Typography,
    storage::{self, BookState, Preferences},
};
use slint::{ComponentHandle, Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, VecModel};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
slint::include_modules!();
#[cfg(feature = "qa")]
mod qa;

struct State {
    request: Option<Request>,
    generation: u64,
    completed_generation: u64,
    frame: Option<Arc<Frame>>,
    right: Option<Arc<Frame>>,
    anchor: u64,
    right_anchor: Option<u64>,
    back: Vec<u64>,
    selected: String,
    full: bool,
    count: i32,
    pagination_complete: bool,
    wheel: WheelPager,
    preferences: Preferences,
    config: PathBuf,
    identity: Option<String>,
    contents: Option<Arc<Vec<sae_reader::document::ContentsItem>>>,
    #[cfg(feature = "qa")]
    metrics: qa::Metrics,
}
fn marks(boxes: Vec<Bounds>) -> ModelRc<Mark> {
    ModelRc::new(VecModel::from(
        boxes
            .into_iter()
            .map(|b| Mark {
                x: b.x,
                y: b.y,
                w: b.w,
                h: b.h,
            })
            .collect::<Vec<_>>(),
    ))
}
fn submit(state: &Arc<Mutex<State>>, mailbox: &Arc<Mailbox>, update: impl FnOnce(&mut Request)) {
    let mut state = state.lock().unwrap();
    if let Some(request) = state.request.as_mut() {
        update(request);
        let request = request.clone();
        state.generation = mailbox.submit(request);
    }
}
fn viewport(ui: &ReaderWindow, request: &mut Request) {
    request.width = ui.get_viewport_width().max(160.0);
    request.height = ui.get_viewport_height().max(160.0);
    request.dpi = ui.window().scale_factor();
    ui.set_effective_spread(request.spread && request.width >= 800.0);
}
fn turn(state: &Arc<Mutex<State>>, mailbox: &Arc<Mailbox>, direction: i32, ui: &ReaderWindow) {
    submit(state, mailbox, |r| {
        let step = if r.spread && ui.get_effective_spread() {
            2
        } else {
            1
        };
        r.page = (r.page + direction * step).max(0);
        r.navigation = Navigation::Page(r.page);
        viewport(ui, r);
    });
}
fn follow(
    uri: String,
    source: Option<u64>,
    ui: &ReaderWindow,
    state: &Arc<Mutex<State>>,
    mailbox: &Arc<Mailbox>,
) {
    if uri.starts_with("https://") || uri.starts_with("http://") {
        let answer = rfd::MessageDialog::new()
            .set_title("在系统浏览器打开外部链接？")
            .set_description(&uri)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer == rfd::MessageDialogResult::Yes {
            let _ = std::process::Command::new("rundll32.exe")
                .args(["url.dll,FileProtocolHandler", &uri])
                .spawn();
        }
        return;
    }
    if uri.contains(':') {
        ui.set_status("此链接协议不受支持".into());
        return;
    }
    {
        let mut s = state.lock().unwrap();
        let a = source.unwrap_or(s.anchor);
        if a > 0 {
            s.back.push(a);
        }
    }
    submit(state, mailbox, |r| r.navigation = Navigation::Link(uri));
}
fn color(hex: &str) -> slint::Color {
    let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xf8f6f1);
    slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}
fn sync_style(ui: &ReaderWindow, style: &Typography) {
    ui.set_font_size_pt(style.size_pt);
    ui.set_line_height(style.line_height);
    ui.set_paragraph_before(style.paragraph_before);
    ui.set_paragraph_after(style.paragraph_after);
    ui.set_indent(style.indent);
    ui.set_text_width(style.width);
    ui.set_justified(style.justified);
    ui.set_margin_top(style.margins[0]);
    ui.set_margin_right(style.margins[1]);
    ui.set_margin_bottom(style.margins[2]);
    ui.set_margin_left(style.margins[3]);
    ui.set_family_label(if style.family == "serif" {
        "默认衬线".into()
    } else {
        style.family.clone().into()
    });
    ui.set_canvas_color(color(&style.background));
    ui.set_dark(style.background.eq_ignore_ascii_case("#171A19"));
    ui.set_show_progress(style.show_progress);
    ui.set_show_toolbar(style.show_toolbar);
}
fn edit_style(state: &Arc<Mutex<State>>, ui: &ReaderWindow, name: &str, value: f32) {
    let mut s = state.lock().unwrap();
    let is_pdf = s.frame.is_some() && !ui.get_reflowable();
    if name == "zoom" || (name == "delta-size" && is_pdf) {
        if let Some(r) = s.request.as_mut() {
            r.pdf_zoom = (r.pdf_zoom
                * if name == "zoom" {
                    value
                } else {
                    1.0 + value * 0.1
                })
            .clamp(0.25, 4.0);
        }
        return;
    }
    let style = &mut s.preferences.typography;
    match name {
        "size" => style.size_pt = value,
        "delta-size" => style.size_pt += value,
        "line" => style.line_height = value,
        "before" => style.paragraph_before = value,
        "paragraph" => style.paragraph_after = value,
        "indent" => style.indent = value,
        "width" => style.width = value,
        "top" => style.margins[0] = value,
        "right" => style.margins[1] = value,
        "bottom" => style.margins[2] = value,
        "left" => style.margins[3] = value,
        "justify" => style.justified = value != 0.0,
        "progress" => style.show_progress = value != 0.0,
        "reset" => *style = Typography::default(),
        _ => {}
    }
    style.validate();
    let style = style.clone();
    if name == "reset" {
        s.preferences.theme = 0;
        ui.set_theme_index(0);
    }
    sync_style(ui, &style);
    if let Some(r) = s.request.as_mut() {
        r.style = style;
    }
}
fn save(state: &Arc<Mutex<State>>) -> anyhow::Result<()> {
    let mut s = state.lock().unwrap();
    if let (Some(identity), Some(request)) = (s.identity.clone(), s.request.as_ref()) {
        let book = BookState {
            path: request.path.clone(),
            anchor: s.anchor,
        };
        s.preferences.books.insert(identity, book);
    }
    s.preferences.save(&s.config)
}

fn run() -> anyhow::Result<()> {
    let boot = Instant::now();
    let portable = std::env::args_os().any(|a| a == "--portable");
    let config = storage::config_path(portable);
    let preferences = Preferences::load(&config);
    let ui = ReaderWindow::new()?;
    sync_style(&ui, &preferences.typography);
    ui.set_double_page(preferences.spread);
    ui.set_theme_index(preferences.theme as i32);
    ui.set_fonts(ModelRc::new(VecModel::from(vec!["默认衬线".into()])));
    if let Ok(variant) = std::env::var("SAE_READER_DESIGN") {
        ui.set_design_variant(variant.parse::<i32>().unwrap_or(0).clamp(0, 2));
    }
    let state = Arc::new(Mutex::new(State {
        request: None,
        generation: 0,
        completed_generation: 0,
        frame: None,
        right: None,
        anchor: 0,
        right_anchor: None,
        back: Vec::new(),
        selected: String::new(),
        full: false,
        count: 0,
        pagination_complete: false,
        wheel: WheelPager::default(),
        preferences,
        config,
        identity: None,
        contents: None,
        #[cfg(feature = "qa")]
        metrics: qa::Metrics::new(boot),
    }));
    let weak = ui.as_weak();
    let shared = state.clone();
    let worker = Worker::start(move |generation, result| {
        let state = shared.clone();
        let _ = weak.upgrade_in_event_loop(move |ui| {
            let mut s = state.lock().unwrap();
            if s.generation != generation {
                return;
            }
            match result {
                Ok(update) => {
                    let frame = &update.frame;
                    let changed = s
                        .frame
                        .as_ref()
                        .is_none_or(|previous| !Arc::ptr_eq(previous, frame))
                        || match (&s.right, &update.right) {
                            (None, None) => false,
                            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                            _ => true,
                        };
                    if changed {
                        ui.set_page_image(Image::from_rgb8(
                            SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
                                &frame.pixels,
                                frame.width,
                                frame.height,
                            ),
                        ));
                        ui.set_image_width(frame.display_width);
                        ui.set_image_height(frame.display_height);
                        if let Some(right) = &update.right {
                            ui.set_right_image(Image::from_rgb8(
                                SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(
                                    &right.pixels,
                                    right.width,
                                    right.height,
                                ),
                            ));
                            ui.set_right_width(right.display_width);
                            ui.set_right_height(right.display_height);
                        } else {
                            ui.set_right_image(Image::default());
                            ui.set_right_width(0.0);
                            ui.set_right_height(0.0);
                        }
                        ui.set_highlights(marks(Vec::new()));
                        ui.set_right_highlights(marks(Vec::new()));
                        s.selected.clear();
                        #[cfg(feature = "qa")]
                        s.metrics.frame(&update);
                    }
                    ui.set_search_marks(marks(update.search));
                    ui.set_right_search_marks(marks(update.right_search));
                    ui.set_has_document(true);
                    ui.set_book_title(update.title.into());
                    ui.set_reflowable(update.reflowable);
                    ui.set_status("".into());
                    ui.set_notice(update.notice.into());
                    if s.contents
                        .as_ref()
                        .is_none_or(|old| !Arc::ptr_eq(old, &update.contents))
                    {
                        ui.set_contents(ModelRc::new(VecModel::from(
                            update
                                .contents
                                .iter()
                                .map(|c| TocRow {
                                    title: format!("{}{}", "  ".repeat(c.depth), c.title).into(),
                                    uri: c.uri.clone().into(),
                                })
                                .collect::<Vec<_>>(),
                        )));
                        s.contents = Some(update.contents.clone());
                    }
                    let last = update.right.as_ref().map(|f| f.page).unwrap_or(frame.page);
                    let range = if last > frame.page {
                        format!("{}–{}", frame.page + 1, last + 1)
                    } else {
                        (frame.page + 1).to_string()
                    };
                    ui.set_page_label(
                        if update.pagination_complete {
                            format!("{range} / {}", update.count)
                        } else {
                            format!("第 {range} 页 · 分页中")
                        }
                        .into(),
                    );
                    ui.set_progress(if update.pagination_complete {
                        (last + 1) as f32 / update.count.max(1) as f32
                    } else {
                        0.0
                    });
                    s.anchor = update.anchor;
                    s.right_anchor = update.right_anchor;
                    s.frame = Some(update.frame.clone());
                    s.right = update.right;
                    s.count = update.count;
                    s.pagination_complete = update.pagination_complete;
                    s.completed_generation = generation;
                    if let Some(request) = s.request.as_mut() {
                        request.page = frame.page;
                        request.navigation = Navigation::Keep;
                    }
                }
                Err(error) => {
                    ui.set_status(format!("无法读取：{error:#}").into());
                    if s.frame.is_none() {
                        ui.set_has_document(false);
                        ui.set_page_image(Image::default());
                        ui.set_right_image(Image::default());
                        ui.set_image_width(0.0);
                    }
                }
            }
        });
    });
    let mailbox = worker.mailbox.clone();
    let resize_timer = Rc::new(slint::Timer::default());
    let style_timer = Rc::new(slint::Timer::default());
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        let timer = resize_timer.clone();
        ui.on_resized(move || {
            let weak = weak.clone();
            let state = state.clone();
            let mailbox = mailbox.clone();
            timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(100),
                move || {
                    if let Some(ui) = weak.upgrade() {
                        let should_submit = {
                            let s = state.lock().unwrap();
                            s.request.as_ref().is_some_and(|r| {
                                (r.width - ui.get_viewport_width()).abs() > 1.0
                                    || (r.height - ui.get_viewport_height()).abs() > 1.0
                                    || (r.dpi - ui.window().scale_factor()).abs() > 0.001
                            })
                        };
                        if should_submit {
                            submit(&state, &mailbox, |r| viewport(&ui, r));
                        }
                    }
                },
            );
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_open_file(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("阅读文档", &["epub", "pdf", "txt", "md"])
                .pick_file()
            {
                if let Some(ui) = weak.upgrade() {
                    open(path, &ui, &state, &mailbox);
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_next_page(move |direction| {
            if let Some(ui) = weak.upgrade() {
                turn(&state, &mailbox, direction, &ui);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_wheel(move |delta| {
            let direction = state.lock().unwrap().wheel.feed(delta, Instant::now());
            if let (Some(direction), Some(ui)) = (direction, weak.upgrade()) {
                turn(&state, &mailbox, direction, &ui);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_mode_changed(move |spread| {
            state.lock().unwrap().preferences.spread = spread;
            if let Some(ui) = weak.upgrade() {
                ui.set_effective_spread(spread && ui.get_viewport_width() >= 800.0);
                submit(&state, &mailbox, |r| {
                    r.spread = spread;
                    r.navigation = Navigation::Keep;
                    viewport(&ui, r);
                });
            }
        });
    }
    {
        let state = state.clone();
        let mailbox = mailbox.clone();
        let weak = ui.as_weak();
        ui.on_follow_link(move |uri| {
            if let Some(ui) = weak.upgrade() {
                follow(uri.to_string(), None, &ui, &state, &mailbox);
            }
        });
    }
    {
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_back(move || {
            let anchor = state.lock().unwrap().back.pop();
            if let Some(a) = anchor {
                submit(&state, &mailbox, |r| r.navigation = Navigation::Anchor(a));
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        let timer = style_timer.clone();
        ui.on_typography(move |name, value| {
            if let Some(ui) = weak.upgrade() {
                edit_style(&state, &ui, name.as_str(), value);
            }
            let state = state.clone();
            let mailbox = mailbox.clone();
            let weak = weak.clone();
            timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(150),
                move || {
                    if let Some(ui) = weak.upgrade() {
                        submit(&state, &mailbox, |r| viewport(&ui, r));
                    }
                },
            );
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_setting_text(move |field, value| {
            if field.as_str() == "family" {
                let mut s = state.lock().unwrap();
                s.preferences.typography.family = if value.as_str() == "默认衬线" {
                    "serif".into()
                } else {
                    value.to_string()
                };
                s.preferences.typography.validate();
                let style = s.preferences.typography.clone();
                if let Some(r) = s.request.as_mut() {
                    r.style = style;
                }
                drop(s);
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_typography("family".into(), 0.0);
                }
            } else if let Ok(value) = value.parse::<f32>() {
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_typography(field, value);
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_theme_changed(move |theme| {
            let (background, foreground) = match theme {
                1 => ("#FFFFFF", "#262A27"),
                2 => ("#171A19", "#E7E9E5"),
                _ => ("#F8F6F1", "#262A27"),
            };
            let mut s = state.lock().unwrap();
            s.preferences.theme = theme.clamp(0, 2) as u8;
            s.preferences.typography.background = background.into();
            s.preferences.typography.foreground = foreground.into();
            let style = s.preferences.typography.clone();
            if let Some(r) = s.request.as_mut() {
                r.style = style;
            }
            drop(s);
            if let Some(ui) = weak.upgrade() {
                ui.set_theme_index(theme);
                ui.invoke_typography("theme".into(), 0.0);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let ready = Rc::new(std::cell::Cell::new(false));
        ui.on_fonts_needed(move || {
            if ready.replace(true) {
                return;
            }
            let mut families = font_kit::source::SystemSource::new()
                .all_families()
                .unwrap_or_default();
            families.sort();
            families.dedup();
            families.insert(0, "默认衬线".into());
            if let Some(ui) = weak.upgrade() {
                ui.set_fonts(ModelRc::new(VecModel::from(
                    families.into_iter().map(Into::into).collect::<Vec<_>>(),
                )));
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_text(move |slot, x0, y0, x1, y1| {
            let mut s = state.lock().unwrap();
            let frame = if slot == 1 {
                s.right.clone()
            } else {
                s.frame.clone()
            };
            if let Some(frame) = frame {
                let (text, boxes) = frame.select((x0, y0), (x1, y1));
                s.selected = text;
                if let Some(ui) = weak.upgrade() {
                    if slot == 1 {
                        ui.set_right_highlights(marks(boxes));
                        ui.set_highlights(marks(Vec::new()));
                    } else {
                        ui.set_highlights(marks(boxes));
                        ui.set_right_highlights(marks(Vec::new()));
                    }
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_hit(move |slot, x, y| {
            let (uri, source) = {
                let s = state.lock().unwrap();
                let frame = if slot == 1 { &s.right } else { &s.frame };
                (
                    frame.as_ref().and_then(|f| {
                        f.links
                            .iter()
                            .find(|l| l.bounds.contains(x, y))
                            .map(|l| l.uri.clone())
                    }),
                    if slot == 1 {
                        s.right_anchor
                    } else {
                        Some(s.anchor)
                    },
                )
            };
            if let (Some(uri), Some(ui)) = (uri, weak.upgrade()) {
                follow(uri, source, &ui, &state, &mailbox);
            }
        });
    }
    {
        let state = state.clone();
        ui.on_copy_text(move || {
            let text = state.lock().unwrap().selected.clone();
            if !text.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                }
            }
        });
    }
    {
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_search(move |text| {
            submit(&state, &mailbox, |r| r.query = text.to_string());
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        ui.on_fullscreen(move || {
            if let Some(ui) = weak.upgrade() {
                let mut s = state.lock().unwrap();
                s.full = !s.full;
                ui.window().set_fullscreen(s.full);
            }
        });
    }
    ui.show()?;
    if let Some(path) = std::env::args_os()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with("--"))
    {
        // Initial Winit DPI/client-size notifications arrive asynchronously.
        // Let that geometry settle once before the first document layout.
        ui.set_status("正在打开…".into());
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        slint::Timer::single_shot(Duration::from_millis(100), move || {
            if let Some(ui) = weak.upgrade() {
                open(PathBuf::from(path), &ui, &state, &mailbox);
            }
        });
    }
    #[cfg(feature = "qa")]
    let _qa = qa::start(&ui, state.clone(), mailbox.clone());
    let _ = boot;
    slint::run_event_loop()?;
    resize_timer.stop();
    style_timer.stop();
    if let Err(error) = save(&state) {
        eprintln!("Cannot save reader state: {error}");
    }
    drop(worker);
    Ok(())
}

fn open(path: PathBuf, ui: &ReaderWindow, state: &Arc<Mutex<State>>, mailbox: &Arc<Mailbox>) {
    ui.set_status("正在打开…".into());
    // Winit supplies the initial client geometry after show(). Do not lay out a
    // fake 160px page and then jump to a second pagination on the first frame.
    if ui.get_viewport_width() < 160.0 || ui.get_viewport_height() < 160.0 {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        slint::Timer::single_shot(Duration::from_millis(10), move || {
            if let Some(ui) = weak.upgrade() {
                open(path, &ui, &state, &mailbox);
            }
        });
        return;
    }
    let _ = save(state);
    ui.set_status("正在打开…".into());
    ui.set_notice("".into());
    ui.set_has_document(false);
    ui.set_page_label("".into());
    let identity = storage::fingerprint(&path).ok();
    let mut s = state.lock().unwrap();
    let anchor = identity
        .as_ref()
        .and_then(|id| s.preferences.books.get(id))
        .map(|b| b.anchor)
        .filter(|a| *a > 0);
    let style = s.preferences.typography.clone();
    let mut request = Request {
        path,
        style,
        width: 0.0,
        height: 0.0,
        dpi: 1.0,
        page: 0,
        navigation: anchor
            .map(Navigation::Anchor)
            .unwrap_or(Navigation::Page(0)),
        query: String::new(),
        pdf_zoom: 1.0,
        spread: s.preferences.spread,
    };
    viewport(ui, &mut request);
    s.back.clear();
    s.frame = None;
    s.right = None;
    s.contents = None;
    s.anchor = 0;
    s.identity = identity;
    s.request = Some(request.clone());
    s.generation = mailbox.submit(request);
}
fn main() {
    if let Err(error) = run() {
        rfd::MessageDialog::new()
            .set_title("Sae Reader 启动失败")
            .set_description(format!("{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
