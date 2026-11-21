#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use sae_reader::{
    paging::WheelPager,
    reading::{Mailbox, Navigation, Request, Worker},
    rendering::{Bounds, Frame},
    settings::Typography,
    storage::{self, BookState, Preferences},
    zoom::{self, Drag, Grip, Position, Stop},
};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
slint::include_modules!();
mod glass;
mod home;
#[cfg(feature = "qa")]
mod qa;

struct State {
    request: Option<Request>,
    generation: u64,
    completed_generation: u64,
    frame: Option<Arc<Frame>>,
    right: Option<Arc<Frame>>,
    extra: Vec<Arc<Frame>>,
    extra_anchors: Vec<u64>,
    anchor: u64,
    right_anchor: Option<u64>,
    back: Vec<u64>,
    selected: String,
    count: i32,
    pagination_complete: bool,
    wheel: WheelPager,
    zoom_drag: Option<(Drag, Grip)>,
    zoom_pending: bool,
    preferences: Preferences,
    config: PathBuf,
    identity: Option<String>,
    contents: Option<Arc<Vec<sae_reader::document::ContentsItem>>>,
    #[cfg(feature = "qa")]
    metrics: qa::Metrics,
}
fn marks(boxes: Vec<Bounds>) -> ModelRc<Mark> {
    if boxes.is_empty() {
        return ModelRc::default();
    }
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
        state.zoom_pending = false;
        state.generation = mailbox.submit(request);
    }
}
fn viewport(ui: &ReaderWindow, request: &mut Request) {
    request.width = ui.get_viewport_width().max(160.0);
    request.height = ui.get_viewport_height().max(160.0);
    request.dpi = ui.window().scale_factor();
}
fn turn(state: &Arc<Mutex<State>>, mailbox: &Arc<Mailbox>, direction: i32, ui: &ReaderWindow) {
    ui.set_selected_content_uri("".into());
    submit(state, mailbox, |r| {
        let step = ui.get_visible_count().max(1);
        let page = r.page.saturating_add(direction * step).max(0);
        navigate(r, ui, page);
    });
}
fn navigate(request: &mut Request, ui: &ReaderWindow, page: i32) {
    request.page = page;
    request.navigation = Navigation::Page(page);
    request.view.pan = [0.5, 0.0];
    request.view.search_hit = None;
    viewport(ui, request);
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
    if uri.split('#').next().unwrap_or("").contains(':') {
        ui.set_status("此链接协议不受支持".into());
        return;
    }
    ui.set_selected_content_uri(uri.clone().into());
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
}
fn submit_zoom(state: &Arc<Mutex<State>>, mailbox: &Arc<Mailbox>, ui: &ReaderWindow) {
    let mut s = state.lock().unwrap();
    if s.zoom_pending && s.generation == s.completed_generation {
        if let Some(request) = s.request.as_mut() {
            viewport(ui, request);
            let request = request.clone();
            s.zoom_pending = false;
            s.generation = mailbox.submit(request);
        }
    }
}
fn edit_zoom(state: &Arc<Mutex<State>>, ui: &ReaderWindow, name: &str, value: f32) -> bool {
    if ui.get_reflowable() || !ui.get_has_document() {
        return false;
    }
    let mut s = state.lock().unwrap();
    let width = ui.get_pdf_fit_width_zoom();
    let fill = ui.get_pdf_fill_zoom();
    let position = match name {
        "fit-width" => Position {
            value: width,
            stop: Stop::Width,
        },
        "fit-cover" => Position {
            value: fill,
            stop: Stop::Fill,
        },
        "fit-page" | "size" => zoom::position(1.0),
        "zoom" => zoom::step(ui.get_pdf_zoom(), fill, value),
        _ => zoom::step(ui.get_pdf_zoom(), fill, 1.12f32.powf(value)),
    };
    apply_zoom(
        &mut s,
        ui,
        position,
        name.starts_with("fit-") || name == "size",
    )
}

fn apply_zoom(s: &mut State, ui: &ReaderWindow, position: Position, reset_pan: bool) -> bool {
    let Some(request) = s.request.as_mut() else {
        return false;
    };
    let fit_width = position.stop == Stop::Width;
    let fit_cover = position.stop == Stop::Fill;
    let zoom = if fit_width || fit_cover {
        1.0
    } else {
        position.value
    };
    let changed = (
        request.view.fit_width,
        request.view.fit_cover,
        request.pdf_zoom,
    ) != (fit_width, fit_cover, zoom);
    request.view.fit_width = fit_width;
    request.view.fit_cover = fit_cover;
    request.pdf_zoom = zoom;
    // A preset click also returns a previously panned page to its starting edge.
    let panned = reset_pan && request.view.pan != [0.5, 0.0];
    if reset_pan {
        request.view.pan = [0.5, 0.0];
    }
    ui.set_pdf_zoom(position.value);
    ui.set_pdf_zoom_stop(position.stop as i32);
    s.zoom_pending |= changed || panned;
    changed || panned
}
fn edit_style(state: &Arc<Mutex<State>>, ui: &ReaderWindow, name: &str, value: f32) {
    let mut s = state.lock().unwrap();
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
            encoding: request.view.encoding.clone(),
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
    let startup_path = std::env::args_os()
        .skip(1)
        .find(|arg| !arg.to_string_lossy().starts_with("--"));
    let ui = ReaderWindow::new()?;
    ui.on_glass(glass::draw);
    ui.on_backdrop(glass::backdrop);
    ui.global::<Materials>().on_face(glass::panel);
    ui.on_choice_index(|model, value| {
        model
            .iter()
            .position(|item| item == value)
            .map_or(-1, |index| index as i32)
    });
    ui.set_opening_document(startup_path.is_some());
    let cover = startup_path
        .is_none()
        .then(|| home::Cover::attach(&ui, &config));
    sync_style(&ui, &preferences.typography);
    ui.set_double_page(preferences.spread);
    ui.set_pdf_multi(preferences.pdf_multi);
    ui.set_grid_choice(preferences.pdf_grid as i32);
    ui.set_theme_index(preferences.theme as i32);
    ui.set_page_color(preferences.page_color as i32);
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
        extra: Vec::new(),
        extra_anchors: Vec::new(),
        anchor: 0,
        right_anchor: None,
        back: Vec::new(),
        selected: String::new(),
        count: 0,
        pagination_complete: false,
        wheel: WheelPager::default(),
        zoom_drag: None,
        zoom_pending: false,
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
                Ok(mut update) => {
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
                    let changed = changed
                        || s.extra.len() != update.extra.len()
                        || s.extra
                            .iter()
                            .zip(&update.extra)
                            .any(|(a, b)| !Arc::ptr_eq(a, b));
                    let layout_changed =
                        ui.get_grid_rows() != update.rows || ui.get_visible_count() != update.step;
                    ui.set_grid_rows(update.rows);
                    ui.set_visible_count(update.step);
                    ui.set_effective_spread(update.step > 1);
                    let previous_tiles = ui.get_pages();
                    // Pagination metadata often follows the same first frame. Reuse the
                    // tile model and its native images on that second notification.
                    let repaint_tiles = changed
                        || layout_changed
                        || !update.search.is_empty()
                        || !update.right_search.is_empty()
                        || update.extra_search.iter().any(|hits| !hits.is_empty())
                        || previous_tiles
                            .iter()
                            .any(|tile| tile.matches.row_count() != 0);
                    if repaint_tiles {
                        let search = std::iter::once(std::mem::take(&mut update.search))
                            .chain(std::iter::once(std::mem::take(&mut update.right_search)))
                            .chain(std::mem::take(&mut update.extra_search));
                        let tiles: Vec<_> = std::iter::once(&update.frame)
                            .chain(update.right.iter())
                            .chain(update.extra.iter())
                            .zip(search)
                            .enumerate()
                            .map(|(i, (frame, hits))| {
                                if !changed && let Some(mut tile) = previous_tiles.row_data(i) {
                                    tile.matches = marks(hits);
                                    return tile;
                                }
                                PageTile {
                                    source: frame.pixels.image(),
                                    w: frame.display_width,
                                    h: frame.display_height,
                                    selection: ModelRc::default(),
                                    matches: marks(hits),
                                }
                            })
                            .collect();
                        if !layout_changed
                            && let Some(model) =
                                previous_tiles.as_any().downcast_ref::<VecModel<PageTile>>()
                        {
                            if model.row_count() == tiles.len() {
                                for (i, tile) in tiles.into_iter().enumerate() {
                                    model.set_row_data(i, tile);
                                }
                            } else {
                                model.set_vec(tiles);
                            }
                        } else {
                            ui.set_pages(ModelRc::new(VecModel::from(tiles)));
                        }
                    }
                    if changed {
                        s.selected.clear();
                        #[cfg(feature = "qa")]
                        s.metrics.frame(&update);
                    }
                    ui.set_has_document(true);
                    ui.set_opening_document(false);
                    ui.set_book_title(update.title.into());
                    ui.set_reflowable(update.reflowable);
                    ui.set_pdf_fit_width_zoom(update.pdf_fit_width_zoom);
                    ui.set_pdf_fill_zoom(update.pdf_fill_zoom);
                    if !update.reflowable {
                        if let Some(request) = &s.request {
                            let actual = request.pdf_zoom
                                * if request.view.fit_cover {
                                    update.pdf_fill_zoom
                                } else if request.view.fit_width {
                                    update.pdf_fit_width_zoom
                                } else {
                                    1.0
                                };
                            let stop = if request.view.fit_cover {
                                Stop::Fill
                            } else if request.view.fit_width {
                                Stop::Width
                            } else {
                                zoom::position(actual).stop
                            };
                            ui.set_pdf_zoom(actual);
                            ui.set_pdf_zoom_stop(stop as i32);
                        }
                    }
                    ui.set_pdf_pannable(
                        !update.reflowable
                            && (frame.overflow.0 > 1.0
                                || frame.overflow.1 > 1.0
                                || update
                                    .extra
                                    .iter()
                                    .any(|f| f.overflow.0 > 1.0 || f.overflow.1 > 1.0)
                                || update
                                    .right
                                    .as_ref()
                                    .is_some_and(|f| f.overflow.0 > 1.0 || f.overflow.1 > 1.0)),
                    );
                    ui.set_encoding_label(update.encoding.unwrap_or_default().into());
                    ui.set_search_status(update.search_status.into());
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
                                    title: c.title.clone().into(),
                                    uri: c.uri.clone().into(),
                                    depth: c.depth.min(12) as i32,
                                    latin: !c.title.chars().any(|ch| {
                                        matches!(ch,
                                        '\u{2e80}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' |
                                        '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
                                    }),
                                })
                                .collect::<Vec<_>>(),
                        )));
                        s.contents = Some(update.contents.clone());
                    }
                    let last = update
                        .extra
                        .last()
                        .or(update.right.as_ref())
                        .map(|f| f.page)
                        .unwrap_or(frame.page);
                    ui.set_first_page(frame.page + 1);
                    ui.set_last_page(last + 1);
                    ui.set_page_total(if update.pagination_complete {
                        update.count
                    } else {
                        0
                    });
                    s.anchor = update.anchor;
                    s.right_anchor = update.right_anchor;
                    s.frame = Some(update.frame.clone());
                    s.right = update.right;
                    s.extra = update.extra;
                    s.extra_anchors = update.extra_anchors;
                    s.count = update.count;
                    s.pagination_complete = update.pagination_complete;
                    s.completed_generation = generation;
                    if let Some(request) = s.request.as_mut() {
                        request.page = frame.page;
                        request.navigation = Navigation::Keep;
                        request.view.search_hit = update.search_hit;
                        // A prior render can finish while a throttled gesture
                        // has newer input waiting. Preserve that input instead
                        // of snapping it back to the older render's position.
                        if request.view.pan == update.input_pan {
                            request.view.pan = update.pan;
                        }
                    }
                }
                Err(error) => {
                    ui.set_opening_document(false);
                    s.completed_generation = generation;
                    ui.set_status(format!("无法读取：{error:#}").into());
                    if s.frame.is_none() {
                        ui.set_has_document(false);
                    }
                }
            }
            let zoom_pending = s.zoom_pending;
            drop(s);
            if zoom_pending {
                ui.invoke_refresh_zoom(false);
            }
        });
    });
    let mailbox = worker.mailbox.clone();
    let resize_timer = Rc::new(slint::Timer::default());
    let style_timer = Rc::new(slint::Timer::default());
    let zoom_timer = Rc::new(slint::Timer::default());
    let pan_timer = Rc::new(slint::Timer::default());
    {
        let state = state.clone();
        let weak = ui.as_weak();
        let mailbox = mailbox.clone();
        let timer = zoom_timer.clone();
        ui.on_refresh_zoom(move |immediate| {
            let Some(ui) = weak.upgrade() else { return };
            if immediate {
                timer.stop();
                submit_zoom(&state, &mailbox, &ui);
                return;
            }
            {
                let s = state.lock().unwrap();
                if !s.zoom_pending || s.generation != s.completed_generation || timer.running() {
                    return;
                }
            }
            // Present each completed tile before submitting the newest zoom.
            // One shot limits work during fine, high-rate pointer movement.
            let state = state.clone();
            let mailbox = mailbox.clone();
            let weak = weak.clone();
            timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(32),
                move || {
                    if let Some(ui) = weak.upgrade() {
                        submit_zoom(&state, &mailbox, &ui);
                    }
                },
            );
        });
    }
    // Capture the monitor in window-local logical coordinates once per press.
    // Only an active linear grip at a physical screen edge owns a running timer.
    let edge_timer = Rc::new(slint::Timer::default());
    let edge_geometry = Rc::new(std::cell::Cell::new(None));
    let edge_sign = Rc::new(std::cell::Cell::new(0_i8));
    {
        let state = state.clone();
        let weak = ui.as_weak();
        let geometry = edge_geometry.clone();
        let timer = edge_timer.clone();
        ui.on_zoom_begin(move |x, y, radius| {
            if let Some(ui) = weak.upgrade() {
                timer.stop();
                geometry.set(
                    ui.window()
                        .with_winit_window(|w| {
                            let monitor = w.current_monitor()?;
                            let client = w.inner_position().ok()?;
                            let offset = monitor.position();
                            let size = monitor.size();
                            let dpi = w.scale_factor() as f32;
                            Some((
                                (client.x - offset.x) as f32 / dpi,
                                (client.y - offset.y) as f32 / dpi,
                                size.width as f32 / dpi,
                                size.height as f32 / dpi,
                            ))
                        })
                        .flatten(),
                );
                state.lock().unwrap().zoom_drag = Some((
                    Drag::start(ui.get_pdf_zoom(), ui.get_pdf_fill_zoom()),
                    Grip::new(x, y, radius),
                ));
            }
        });
    }
    {
        let state = state.clone();
        let weak = ui.as_weak();
        let timer = edge_timer.clone();
        ui.on_zoom_motion(move |x, y, window_x, window_y| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let mut s = state.lock().unwrap();
            let Some((dial, grip)) = s.zoom_drag.as_mut() else {
                return false;
            };
            let Some(travel) = grip.sample(x, y, *dial) else {
                return false;
            };
            let sign = edge_geometry.get().map_or(0, |(ox, oy, w, h)| {
                grip.edge_direction(window_x + ox, window_y + oy, w, h)
            });
            let position = dial.at(travel);
            let changed = apply_zoom(&mut s, &ui, position, false);
            drop(s);
            if changed {
                ui.invoke_refresh_zoom(false);
            }
            let previous = edge_sign.replace(sign);
            if sign == 0 {
                timer.stop();
            } else if sign != previous || !timer.running() {
                let state = state.clone();
                let weak = weak.clone();
                let running = Rc::downgrade(&timer);
                timer.start(
                    slint::TimerMode::Repeated,
                    Duration::from_millis(32),
                    move || {
                        let Some(ui) = weak.upgrade() else { return };
                        let mut s = state.lock().unwrap();
                        let next = s
                            .zoom_drag
                            .as_mut()
                            .and_then(|(dial, grip)| grip.advance(8.0 * f32::from(sign), *dial));
                        if let Some(next) = next {
                            let changed = apply_zoom(&mut s, &ui, next, false);
                            drop(s);
                            if changed {
                                ui.invoke_refresh_zoom(false);
                            }
                        } else if let Some(timer) = running.upgrade() {
                            timer.stop();
                        }
                    },
                );
            }
            true
        });
    }
    {
        let state = state.clone();
        let weak = ui.as_weak();
        ui.on_zoom_end(move || {
            edge_timer.stop();
            state.lock().unwrap().zoom_drag = None;
            if let Some(ui) = weak.upgrade() {
                ui.invoke_refresh_zoom(true);
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        let timer = pan_timer.clone();
        ui.on_pan(move |dx, dy| {
            {
                let mut s = state.lock().unwrap();
                let overflow = s
                    .frame
                    .iter()
                    .chain(s.right.iter())
                    .chain(s.extra.iter())
                    .fold((0.0f32, 0.0f32), |a, f| {
                        (a.0.max(f.overflow.0), a.1.max(f.overflow.1))
                    });
                if let Some(r) = s.request.as_mut() {
                    if overflow.0 > 1.0 {
                        r.view.pan[0] = (r.view.pan[0] + dx / overflow.0).clamp(0.0, 1.0);
                    }
                    if overflow.1 > 1.0 {
                        r.view.pan[1] = (r.view.pan[1] + dy / overflow.1).clamp(0.0, 1.0);
                    }
                }
            }
            // Coalesce to one pending refresh without postponing it throughout
            // a long drag. The mailbox always receives the latest pan position.
            if timer.running() {
                return;
            }
            let state = state.clone();
            let mailbox = mailbox.clone();
            let weak = weak.clone();
            timer.start(
                slint::TimerMode::SingleShot,
                Duration::from_millis(40),
                move || {
                    if weak.upgrade().is_some() {
                        submit(&state, &mailbox, |_| {});
                    }
                },
            );
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_page_color_changed(move |mode| {
            let mode = mode.clamp(0, 2) as u8;
            state.lock().unwrap().preferences.page_color = mode;
            if let Some(ui) = weak.upgrade() {
                ui.set_page_color(mode as i32);
            }
            submit(&state, &mailbox, |r| r.view.color = mode);
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_encoding_changed(move |encoding| {
            if let Some(ui) = weak.upgrade() {
                ui.set_status("正在切换编码…".into());
                ui.set_encoding_choice(encoding.clone());
            }
            submit(&state, &mailbox, |r| {
                r.view.encoding = if encoding.as_str() == "自动检测" {
                    None
                } else {
                    Some(encoding.to_string())
                };
                r.navigation = Navigation::Page(0);
                r.view.search_hit = None;
            });
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        let timer = resize_timer.clone();
        ui.on_resized(move || {
            // During a free window drag collect nearby sizes. A settled
            // fullscreen monitor has a known destination and can render now.
            let at_full_size = weak.upgrade().is_some_and(|ui| {
                ui.get_is_fullscreen()
                    && ui
                        .window()
                        .with_winit_window(|w| {
                            w.current_monitor()
                                .is_some_and(|m| w.inner_size() == m.size())
                        })
                        .unwrap_or(false)
            });
            let weak = weak.clone();
            let state = state.clone();
            let mailbox = mailbox.clone();
            timer.start(
                slint::TimerMode::SingleShot,
                if at_full_size {
                    Duration::ZERO
                } else {
                    Duration::from_millis(100)
                },
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
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let home = ui.get_home_session();
            if home {
                ui.invoke_home_picker(true);
            }
            if let Some(path) = rfd::FileDialog::new()
                .add_filter(
                    "电子书与文档",
                    &["epub", "pdf", "txt", "md", "azw3", "mobi", "fb2"],
                )
                .pick_file()
            {
                open(path, &ui, &state, &mailbox);
            } else if home {
                ui.invoke_home_picker(false);
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
        ui.on_jump_page(move |text| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            let Ok(page) = text.trim().parse::<i32>() else {
                return false;
            };
            if !ui.get_has_document() || !(1..=ui.get_page_total()).contains(&page) {
                return false;
            }
            ui.set_selected_content_uri("".into());
            submit(&state, &mailbox, |r| navigate(r, &ui, page - 1));
            true
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_wheel_xy(move |dx, dy, shift, control| {
            if let Some(ui) = weak.upgrade() {
                if control && !ui.get_reflowable() {
                    ui.invoke_typography("zoom".into(), 1.12f32.powf(dy / 120.0));
                } else if ui.get_pdf_pannable() && (shift || dx.abs() > 0.0) {
                    ui.invoke_pan(if shift { -dy } else { -dx }, if shift { 0.0 } else { -dy });
                } else {
                    ui.invoke_wheel(dy);
                }
            }
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_wheel(move |delta| {
            if let Some(ui) = weak.upgrade() {
                if ui.get_pdf_pannable() {
                    ui.invoke_pan(0.0, -delta);
                    return;
                }
            }
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
            {
                let mut s = state.lock().unwrap();
                s.preferences.spread = spread;
                s.preferences.pdf_multi = false;
            }
            if let Some(ui) = weak.upgrade() {
                ui.set_pdf_multi(false);
                submit(&state, &mailbox, |r| {
                    r.spread = spread;
                    r.view.multi = false;
                    r.navigation = Navigation::Keep;
                    viewport(&ui, r);
                });
            }
        });
    }
    {
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_grid_changed(move |choice| {
            let grid = choice.clamp(0, sae_reader::paging::PDF_GRIDS.len() as i32) as u8;
            state.lock().unwrap().preferences.pdf_grid = grid;
            submit(&state, &mailbox, |r| {
                r.view.grid = grid;
                r.navigation = Navigation::Keep;
            });
        });
    }
    {
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        ui.on_multi_mode(move || {
            state.lock().unwrap().preferences.pdf_multi = true;
            if let Some(ui) = weak.upgrade() {
                submit(&state, &mailbox, |r| {
                    r.view.multi = true;
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
            let Some(ui) = weak.upgrade() else { return };
            let fixed_action = name.as_str() == "zoom"
                || name.starts_with("fit-")
                || !ui.get_reflowable() && matches!(name.as_str(), "size" | "delta-size");
            if fixed_action {
                if edit_zoom(&state, &ui, name.as_str(), value) {
                    ui.invoke_refresh_zoom(name.starts_with("fit-") || name == "size");
                }
                return;
            }
            edit_style(&state, &ui, name.as_str(), value);
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
            let frame = match slot {
                0 => s.frame.clone(),
                1 => s.right.clone(),
                _ => s.extra.get((slot - 2) as usize).cloned(),
            };
            if let Some(frame) = frame {
                let (text, boxes) = frame.select((x0, y0), (x1, y1));
                s.selected = text;
                if let Some(ui) = weak.upgrade() {
                    let tiles = ui.get_pages();
                    if let Some(model) = tiles.as_any().downcast_ref::<VecModel<PageTile>>() {
                        let mut selection = marks(boxes);
                        for i in 0..slint::Model::row_count(model) {
                            if let Some(mut tile) = slint::Model::row_data(model, i) {
                                tile.selection = if i == slot as usize {
                                    std::mem::take(&mut selection)
                                } else if tile.selection.row_count() != 0 {
                                    ModelRc::default()
                                } else {
                                    continue;
                                };
                                slint::Model::set_row_data(model, i, tile);
                            }
                        }
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
            let (uri, source, width) = {
                let s = state.lock().unwrap();
                let frame = match slot {
                    0 => s.frame.as_ref(),
                    1 => s.right.as_ref(),
                    _ => s.extra.get((slot - 2) as usize),
                };
                (
                    frame.as_ref().and_then(|f| {
                        f.links
                            .iter()
                            .find(|l| l.bounds.contains(x, y))
                            .map(|l| l.uri.clone())
                    }),
                    match slot {
                        0 => Some(s.anchor),
                        1 => s.right_anchor,
                        _ => s.extra_anchors.get((slot - 2) as usize).copied(),
                    },
                    frame.as_ref().map_or(0.0, |f| f.display_width),
                )
            };
            if let Some(ui) = weak.upgrade() {
                if let Some(uri) = uri {
                    follow(uri, source, &ui, &state, &mailbox);
                } else {
                    let direction =
                        sae_reader::paging::click_direction(ui.get_grid_columns(), slot, x, width);
                    turn(&state, &mailbox, direction, &ui);
                }
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
        let weak = ui.as_weak();
        ui.on_search(move |text| {
            let query: String = text.chars().filter(|c| *c != '\0').take(256).collect();
            if query.is_empty()
                && state
                    .lock()
                    .unwrap()
                    .request
                    .as_ref()
                    .is_none_or(|r| r.query.is_empty())
            {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.set_status(
                    if query.is_empty() {
                        ""
                    } else {
                        "正在全文查找…"
                    }
                    .into(),
                );
            }
            submit(&state, &mailbox, |r| {
                if query != r.query {
                    r.view.search_hit = None;
                }
                r.query = query;
                r.navigation = if r.query.is_empty() {
                    Navigation::Keep
                } else {
                    Navigation::Search(1)
                };
            });
        });
    }
    {
        let state = state.clone();
        let mailbox = mailbox.clone();
        let weak = ui.as_weak();
        ui.on_search_next(move |direction| {
            if state
                .lock()
                .unwrap()
                .request
                .as_ref()
                .is_none_or(|r| r.query.is_empty())
            {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.set_status("正在全文查找…".into());
            }
            submit(&state, &mailbox, |r| {
                r.navigation = Navigation::Search(direction)
            });
        });
    }
    {
        let weak = ui.as_weak();
        let restore_normal = std::cell::Cell::new(false);
        ui.on_fullscreen(move || {
            if let Some(ui) = weak.upgrade() {
                let enter = !ui.get_is_fullscreen();
                ui.set_is_fullscreen(enter);
                if enter {
                    // Windows keeps the normal placement even when maximized.
                    // Restoring here would briefly display that smaller window.
                    restore_normal.set(ui.window().is_maximized());
                }
                ui.window().set_fullscreen(enter);
                if !enter && restore_normal.replace(false) {
                    ui.window().with_winit_window(|native| {
                        native.set_fullscreen(None);
                        native.set_maximized(false);
                    });
                    ui.window().set_maximized(false);
                }
                ui.invoke_focus_reading();
            }
        });
    }
    {
        let weak = ui.as_weak();
        let cover = cover.clone();
        let mut modifiers = winit::keyboard::ModifiersState::empty();
        let mut maximize_pending = false;
        // Window-level shortcuts work while a button, slider or text field owns
        // focus. Cursor events only update overlay visibility, never layout.
        // Scripted gestures target the native Slint window even while it is
        // in the background. Real desktop input and focus loss belong to the
        // physical device, and would otherwise cancel a simulated gesture.
        #[cfg(feature = "qa")]
        let scripted_pointer = std::env::var_os("SAE_READER_QA_REPORT").is_some()
            && matches!(
                std::env::var("SAE_READER_QA_SCENARIO").as_deref(),
                Ok("chrome" | "detents" | "instruments" | "navigation" | "visual")
            );
        ui.window().on_winit_window_event(move |window, event| {
            let Some(ui) = weak.upgrade() else {
                return EventResult::Propagate;
            };
            if let Some(cover) = &cover {
                cover.event(&ui, event);
            }
            #[cfg(feature = "qa")]
            if scripted_pointer
                && matches!(
                    event,
                    winit::event::WindowEvent::CursorMoved { .. }
                        | winit::event::WindowEvent::CursorLeft { .. }
                        | winit::event::WindowEvent::CursorEntered { .. }
                        | winit::event::WindowEvent::MouseInput { .. }
                        | winit::event::WindowEvent::MouseWheel { .. }
                        | winit::event::WindowEvent::KeyboardInput { .. }
                        | winit::event::WindowEvent::Focused(false)
                )
            {
                return EventResult::PreventDefault;
            }
            match event {
                winit::event::WindowEvent::ModifiersChanged(event) => {
                    modifiers = event.state();
                }
                winit::event::WindowEvent::Focused(false) => {
                    modifiers = winit::keyboard::ModifiersState::empty();
                    ui.set_top_hover(false);
                    ui.set_bottom_hover(false);
                    ui.set_zoom_hover(false);
                    ui.set_zoom_pinned(false);
                    if ui.get_zoom_dragging() {
                        ui.set_zoom_dragging(false);
                        ui.invoke_zoom_end();
                    }
                    ui.set_controls_busy(false);
                }
                winit::event::WindowEvent::Resized(_)
                    if !ui.get_is_fullscreen()
                        && window
                            .with_winit_window(|native| native.is_maximized())
                            .unwrap_or(false) =>
                {
                    if !maximize_pending {
                        maximize_pending = true;
                        let weak = weak.clone();
                        // Let Slint finish synchronizing the native state before changing it.
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = weak.upgrade() {
                                if ui.window().is_maximized() && !ui.get_is_fullscreen() {
                                    ui.invoke_fullscreen();
                                }
                            }
                        });
                    }
                }
                winit::event::WindowEvent::Resized(_) => maximize_pending = false,
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    let scale = window.scale_factor() as f64;
                    ui.invoke_edge_hover((position.x / scale) as f32, (position.y / scale) as f32);
                }
                winit::event::WindowEvent::CursorLeft { .. } => {
                    ui.set_top_hover(false);
                    ui.set_bottom_hover(false);
                    if !ui.get_zoom_dragging() {
                        ui.set_zoom_hover(false);
                    }
                }
                winit::event::WindowEvent::KeyboardInput { event, .. }
                    if event.state == winit::event::ElementState::Pressed =>
                {
                    use winit::keyboard::{Key, NamedKey};
                    if !modifiers.control_key()
                        && !modifiers.alt_key()
                        && !modifiers.super_key()
                        && ui.get_reading_keys()
                    {
                        let direction = match event.logical_key {
                            Key::Named(NamedKey::ArrowLeft | NamedKey::PageUp) => -1,
                            Key::Named(NamedKey::ArrowRight | NamedKey::PageDown) => 1,
                            _ => 0,
                        };
                        if direction != 0 {
                            ui.invoke_next_page(direction);
                            return EventResult::PreventDefault;
                        }
                    }
                    if event.repeat {
                        return EventResult::Propagate;
                    }
                    if modifiers.control_key() {
                        use winit::keyboard::{KeyCode, PhysicalKey};
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::KeyO) => {
                                ui.invoke_open_file();
                                return EventResult::PreventDefault;
                            }
                            PhysicalKey::Code(KeyCode::KeyF) => {
                                ui.invoke_focus_search();
                                return EventResult::PreventDefault;
                            }
                            PhysicalKey::Code(KeyCode::KeyG) => {
                                ui.invoke_focus_page_entry();
                                return EventResult::PreventDefault;
                            }
                            _ => {}
                        }
                    }
                    match event.logical_key {
                        Key::Named(NamedKey::F11) => {
                            ui.invoke_fullscreen();
                            return EventResult::PreventDefault;
                        }
                        Key::Named(NamedKey::F10) => {
                            ui.set_show_toolbar(!ui.get_show_toolbar());
                            return EventResult::PreventDefault;
                        }
                        Key::Named(NamedKey::Escape) => {
                            if ui.get_choice_open() {
                                return EventResult::Propagate;
                            }
                            if ui.get_page_entry_active() {
                                ui.invoke_focus_reading();
                            } else if ui.get_show_contents()
                                || ui.get_show_settings()
                                || ui.get_show_search()
                                || ui.get_show_zoom_pop()
                            {
                                ui.invoke_dismiss_panels();
                            } else if ui.get_is_fullscreen() {
                                ui.invoke_fullscreen();
                            } else {
                                ui.invoke_dismiss_panels();
                            }
                            return EventResult::PreventDefault;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            EventResult::Propagate
        });
    }
    ui.show()?;
    if let Some(cover) = &cover {
        cover.shown(&ui);
    }
    if let Some(path) = startup_path {
        // Defer to the event loop, then compare Slint's viewport with the real
        // native client area. The old unconditional 100ms floor penalized every
        // command-line open even when both were already settled. Keep that
        // one-shot fallback for a late geometry notification; never poll idle.
        ui.set_status("正在打开…".into());
        let weak = ui.as_weak();
        let state = state.clone();
        let mailbox = mailbox.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(ui) = weak.upgrade() {
                let dpi = ui.window().scale_factor().max(0.1);
                let settled = ui
                    .window()
                    .with_winit_window(|window| {
                        let native = window.inner_size();
                        (native.width as f32 / dpi - ui.get_viewport_width()).abs() < 2.0
                            && (native.height as f32 / dpi - ui.get_viewport_height()).abs() < 2.0
                            && ui.get_viewport_width() >= 160.0
                            && ui.get_viewport_height() >= 160.0
                    })
                    .unwrap_or(false);
                if settled {
                    open(PathBuf::from(path), &ui, &state, &mailbox);
                } else {
                    slint::Timer::single_shot(Duration::from_millis(100), move || {
                        if let Some(ui) = weak.upgrade() {
                            open(PathBuf::from(path), &ui, &state, &mailbox);
                        }
                    });
                }
            }
        });
    }
    #[cfg(feature = "qa")]
    let _qa = qa::start(&ui, state.clone(), mailbox.clone());
    let _ = boot;
    slint::run_event_loop()?;
    resize_timer.stop();
    style_timer.stop();
    zoom_timer.stop();
    pan_timer.stop();
    if let Err(error) = save(&state) {
        eprintln!("Cannot save reader state: {error}");
    }
    drop(worker);
    Ok(())
}

fn open(path: PathBuf, ui: &ReaderWindow, state: &Arc<Mutex<State>>, mailbox: &Arc<Mailbox>) {
    if ui.get_home_session() {
        ui.invoke_retire_home();
    }
    ui.set_opening_document(true);
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
    ui.set_page_entry_active(false);
    ui.set_pages(ModelRc::default());
    ui.set_text_document(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt") || e.eq_ignore_ascii_case("md")),
    );
    ui.set_pdf_zoom(1.0);
    ui.set_pdf_fit_width_zoom(1.0);
    ui.set_pdf_fill_zoom(1.0);
    ui.set_pdf_zoom_stop(1);
    ui.set_zoom_pinned(false);
    ui.set_pdf_pannable(false);
    ui.set_search_status("".into());
    ui.set_page_total(0);
    ui.set_selected_content_uri("".into());
    let identity = storage::fingerprint(&path).ok();
    let mut s = state.lock().unwrap();
    s.zoom_drag = None;
    s.zoom_pending = false;
    let anchor = identity
        .as_ref()
        .and_then(|id| s.preferences.books.get(id))
        .map(|b| b.anchor)
        .filter(|a| *a > 0);
    let style = s.preferences.typography.clone();
    let saved_encoding = identity
        .as_ref()
        .and_then(|id| s.preferences.books.get(id))
        .and_then(|b| b.encoding.clone());
    ui.set_encoding_choice(
        saved_encoding
            .clone()
            .unwrap_or_else(|| "自动检测".to_owned())
            .into(),
    );
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
        view: Default::default(),
        spread: s.preferences.spread,
    };
    viewport(ui, &mut request);
    request.view.color = s.preferences.page_color;
    request.view.multi = s.preferences.pdf_multi;
    request.view.grid = s.preferences.pdf_grid;
    request.view.encoding = saved_encoding;
    s.back.clear();
    s.frame = None;
    s.right = None;
    s.extra.clear();
    s.extra_anchors.clear();
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
