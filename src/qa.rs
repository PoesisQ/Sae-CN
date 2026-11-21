//! Explicit, optional native-window diagnostics. Never enabled by a release build.
//! Reports contain timings and counters, never book paths, titles or text.
use super::*;
mod opening;
pub(crate) use opening::{choose_file, opening_motion_policy};
mod page_input;
use serde_json::{Value, json};
use slint::Model;
use std::{
    cell::{Cell, RefCell},
    fs,
    io::BufWriter,
    path::Path,
};

#[derive(Clone, Copy, serde::Serialize)]
struct PointerSample {
    client_px: Option<(i32, i32)>,
    left_down: bool,
    wheel_events: u32,
}

thread_local! {
    static POINTER: Cell<PointerSample> = const { Cell::new(PointerSample {
        client_px: None, left_down: false, wheel_events: 0
    }) };
}

pub fn observe_pointer_event(event: &winit::event::WindowEvent) {
    use winit::event::{ElementState, MouseButton, WindowEvent};
    POINTER.with(|pointer| {
        let mut sample = pointer.get();
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                sample.client_px = Some((position.x.round() as i32, position.y.round() as i32));
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                sample.left_down = *state == ElementState::Pressed;
            }
            WindowEvent::MouseWheel { .. } => {
                sample.wheel_events = sample.wheel_events.wrapping_add(1)
            }
            _ => return,
        }
        pointer.set(sample);
    });
}

pub struct Metrics {
    boot: Instant,
    first_frame_ms: Option<f64>,
    frames: Vec<Value>,
    max_cache: usize,
}
impl Metrics {
    pub fn new(boot: Instant) -> Self {
        Self {
            boot,
            first_frame_ms: None,
            frames: Vec::new(),
            max_cache: 0,
        }
    }
    pub fn frame(&mut self, u: &sae_reader::reading::Update) {
        let now = self.boot.elapsed().as_secs_f64() * 1000.0;
        self.first_frame_ms.get_or_insert(now);
        self.max_cache = self.max_cache.max(u.cache_bytes);
        self.frames.push(
            json!({"at_ms":now,"worker_ms":u.elapsed_ms,"page":u.frame.page,
            "pixels":[u.frame.width,u.frame.height],"display":[u.frame.display_width,u.frame.display_height],"dpi":u.frame.dpi,
            "right_page":u.right.as_ref().map(|f|f.page),"extra_pages":u.extra.iter().map(|f|f.page).collect::<Vec<_>>(),"rows":u.rows,"step":u.step,"cache_bytes":u.cache_bytes,
            "cache_hits":u.cache_hit,"cache_misses":u.cache_miss}),
        );
    }
}

#[repr(C)]
#[derive(Default)]
struct Memory {
    cb: u32,
    faults: u32,
    peak_ws: usize,
    working_set: usize,
    peak_paged: usize,
    paged: usize,
    peak_nonpaged: usize,
    nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private_bytes: usize,
    private_ws: usize,
    shared_commit: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn GetProcessTimes(
        process: *mut std::ffi::c_void,
        create: *mut u64,
        exit: *mut u64,
        kernel: *mut u64,
        user: *mut u64,
    ) -> i32;
    fn GetProcessHandleCount(process: *mut std::ffi::c_void, count: *mut u32) -> i32;
}
#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(process: *mut std::ffi::c_void, memory: *mut Memory, size: u32) -> i32;
}
fn counters() -> Value {
    let mut m = Memory {
        cb: std::mem::size_of::<Memory>() as u32,
        ..Default::default()
    };
    let (mut create, mut exit, mut kernel, mut user, mut handles) = (0, 0, 0, 0, 0);
    unsafe {
        let p = GetCurrentProcess();
        GetProcessMemoryInfo(p, &mut m, std::mem::size_of::<Memory>() as u32);
        GetProcessTimes(p, &mut create, &mut exit, &mut kernel, &mut user);
        GetProcessHandleCount(p, &mut handles);
    }
    json!({"private_bytes":m.private_bytes,"private_working_set":m.private_ws,
        "working_set":m.working_set,"peak_working_set":m.peak_ws,"peak_pagefile":m.peak_pagefile,
        "cpu_ms":(kernel+user) as f64/10000.0,"handles":handles})
}
fn snapshot(ui: &ReaderWindow, path: &Path) -> anyhow::Result<()> {
    write_snapshot(&ui.window().take_snapshot()?, path)
}

fn write_snapshot(
    pixels: &slint::SharedPixelBuffer<slint::Rgba8Pixel>,
    path: &Path,
) -> anyhow::Result<()> {
    let file = BufWriter::new(fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()?
        .write_image_data(pixels.as_bytes())?;
    Ok(())
}

fn cover_snapshot(ui: &ReaderWindow, path: &Path) -> anyhow::Result<()> {
    snapshot(ui, path)?;
    if let Some(field) = ui.get_home_frame().to_rgb8() {
        // Both captures happen in one event. This is the exact input to the
        // native compositor, even if the next animation frame is due.
        let file = fs::File::create(path.with_extension("field.png"))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), field.width(), field.height());
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(field.as_bytes())?;
    }
    Ok(())
}

#[derive(Default)]
struct Run {
    ready: Option<Instant>,
    painted_ms: Option<f64>,
    first_counters: Option<Value>,
    idle_start: Option<Value>,
    idle_at: Option<Instant>,
    snapshots: Vec<Value>,
    step: usize,
    waiting: bool,
    operations: usize,
    baseline: Option<Value>,
    end: Option<Value>,
    errors: Vec<String>,
    chrome_viewport: Option<(f32, f32)>,
    pan_origin: f32,
    panel_generation: Option<u64>,
    home_before: i32,
    dial_last: Option<f32>,
    dial_stops: [usize; 4],
    dial_signature: Option<(f32, blake3::Hash)>,
    dial_rotation_samples: usize,
    navigation_expected: Option<i32>,
}
/// A QA-only timer is created only when an explicit report destination is set.
/// The production binary has no diagnostic timer, environment parser or PNG codec.
pub fn start(
    ui: &ReaderWindow,
    state: Arc<Mutex<State>>,
    mailbox: Arc<Mailbox>,
) -> Option<Rc<slint::Timer>> {
    let report = PathBuf::from(std::env::var_os("SAE_READER_QA_REPORT")?);
    let scenario = std::env::var("SAE_READER_QA_SCENARIO").unwrap_or_else(|_| "idle".into());
    let screenshots = std::env::var_os("SAE_READER_QA_SCREENSHOTS").map(PathBuf::from);
    let weak = ui.as_weak();
    let run = Rc::new(RefCell::new(Run::default()));
    let mut opening = (scenario == "opening").then(opening::Probe::new);
    let timer = Rc::new(slint::Timer::default());
    let stop = Rc::downgrade(&timer);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(25),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let mut run = run.borrow_mut();
            if let Some(probe) = &mut opening {
                match probe.tick(&ui, &state, &mailbox, screenshots.as_deref(), &mut run) {
                    Ok(false) => return,
                    Ok(true) => {}
                    Err(error) => run.errors.push(error.to_string()),
                }
                finish(&ui, &state, &report, &scenario, &run);
                if let Some(timer) = stop.upgrade() { timer.stop(); }
                return;
            }
            if ui.get_status().starts_with("无法读取") {
                run.errors.push("document open/render failed; see local stderr".into());
                finish(&ui,&state,&report,&scenario,&run);
                if let Some(t)=stop.upgrade(){t.stop();}
                return;
            }
            let (ready, boot) = {
                let s = state.lock().unwrap();
                (
                    s.request.is_none() && ui.get_status().is_empty()
                        || s.frame.is_some()
                            && s.generation == s.completed_generation
                            && !s.zoom_pending,
                    s.metrics.boot,
                )
            };
            if boot.elapsed() > Duration::from_secs(180) {
                run.errors.push("scenario timeout".into());
                finish(&ui, &state, &report, &scenario, &run);
                if let Some(t) = stop.upgrade() {
                    t.stop();
                }
                return;
            }
            // The Win32 input regression driver posts actual window messages.
            // Sampling/PNG capture are opt-in and compiled only in the QA build.
            if scenario == "wheel-input" && run.ready.is_some() {
                let request = report.with_extension("probe");
                if let Ok(command) = fs::read_to_string(&request) {
                    let _ = fs::remove_file(request);
                    let command = command.trim();
                    if command == "finish" {
                        finish(&ui, &state, &report, &scenario, &run);
                        if let Some(t) = stop.upgrade() { t.stop(); }
                        return;
                    }
                    if command == "dark" { ui.invoke_theme_changed(2); }
                    if command.starts_with("capture-") {
                        if let Some(folder) = &screenshots {
                            if let Err(e) = snapshot(&ui, &folder.join(format!("wheel-{}.png", run.operations))) {
                                run.errors.push(e.to_string());
                            }
                        }
                    }
                    let s = state.lock().unwrap();
                    let value = json!({"seq":command,"sample":run.operations,"dpi":ui.window().scale_factor(),
                        "pointer":POINTER.with(Cell::get),
                        "window":[ui.get_viewport_width(),ui.get_viewport_height()],
                        "mode_x":ui.get_mode_center_x(),"header":ui.get_header_height_logical(),"depth":ui.get_mode_depth(),
                        "mode_position":ui.get_mode_position(),"mode_held":ui.get_mode_dragging(),
                        "presets":[ui.get_preset_page_x(),ui.get_preset_page_y(),ui.get_preset_width_y()],
                        "mode":if ui.get_pdf_multi(){2}else if ui.get_double_page(){1}else{0},
                        "dock_right":ui.get_dock_right(),"dock_top":ui.get_dock_top(),
                        "center":[ui.get_knob_center_x(),ui.get_knob_center_y()],"radius":ui.get_knob_radius(),
                        "zoom":ui.get_pdf_zoom(),"fill":ui.get_pdf_fill_zoom(),"width_zoom":ui.get_pdf_fit_width_zoom(),
                        "pinned":ui.get_zoom_pinned(),"pressed":ui.get_zoom_dragging(),
                        "page":s.frame.as_ref().map(|f|f.page),"generation":s.generation});
                    drop(s);
                    let _ = fs::write(report.with_extension("sample"), value.to_string());
                    run.operations += 1;
                }
                return;
            }
            if !ready {
                return;
            }
            if run.ready.is_none() {
                // Force the actual native renderer to paint the assigned page.
                match ui.window().take_snapshot() {
                    Ok(_) => {}
                    Err(e) => run.errors.push(e.to_string()),
                }
                run.painted_ms = Some(boot.elapsed().as_secs_f64() * 1000.0);
                run.ready = Some(Instant::now());
                run.first_counters = Some(counters());
                if let Some(folder) = &screenshots {
                    let _ = fs::create_dir_all(folder);
                    if let Err(e) = snapshot(&ui, &folder.join("first.png")) {
                        run.errors.push(e.to_string());
                    }
                }
                return;
            }
            if Instant::now() < run.ready.unwrap() {
                return;
            }
            if scenario == "home-motion" {
                let step = run.step;
                let count = ui.get_home_frames();
                // Windows hands over focus asynchronously, including after restore.
                if step == 1 && count == 0 && run.ready.unwrap().elapsed() < Duration::from_secs(4) {
                    ui.window().with_winit_window(|w| w.focus_window());
                    return;
                }
                run.snapshots.push(json!({"step":step,"frames":count,"cover":ui.get_home_variant(),
                    "texture":ui.get_home_frame().size().width,"home_session":ui.get_home_session()}));
                let next = || {
                    use slint::platform::{Key,WindowEvent};
                    ui.window().dispatch_event(WindowEvent::KeyPressed{text:Key::RightArrow.into()});
                    ui.window().dispatch_event(WindowEvent::KeyReleased{text:Key::RightArrow.into()});
                };
                let capture = |name:&str| -> anyhow::Result<()> {
                    if let Some(dir)=&screenshots { cover_snapshot(&ui,&dir.join(name))?; }
                    Ok(())
                };
                let action = match step {
                    0 => {
                        ui.window().with_winit_window(|w| w.focus_window());
                        Ok(())
                    }
                    1 => {
                        if count==0 || ui.get_home_variant()!=1 { run.errors.push("first cover did not animate".into()); }
                        capture("plasma.png")
                    }
                    2 => { let image=capture("plasma-later.png"); next(); image }
                    3 => {
                        if count<=run.home_before || ui.get_home_variant()!=2 { run.errors.push("second cover did not animate".into()); }
                        let image=capture("blue.png");
                        ui.invoke_home_picker(true);
                        image
                    }
                    4 => {
                        if count!=run.home_before { run.errors.push("cover ran underneath the file dialog".into()); }
                        ui.invoke_home_picker(false); Ok(())
                    }
                    5 => {
                        if count<=run.home_before { run.errors.push("cancelled picker did not resume".into()); }
                        let image=capture("blue-later.png"); ui.window().set_minimized(true); image
                    }
                    6 => { Ok(()) }
                    7 => {
                        if count!=run.home_before { run.errors.push("minimized cover timer advanced".into()); }
                        ui.window().set_minimized(false);
                        ui.window().with_winit_window(|w| w.focus_window()); Ok(())
                    }
                    8 => { next(); Ok(()) }
                    9 => {
                        if ui.get_home_variant()!=0 || ui.get_home_frame().size().width!=0 {run.errors.push("print retained a motion texture".into());}
                        let image=capture("print.png");next();image
                    }
                    10 => {
                        if count<=run.home_before { run.errors.push("cover failed to resume after print".into()); }
                        open(PathBuf::from("tests/fixtures/core.pdf"),&ui,&state,&mailbox);Ok(())
                    }
                    _ => {
                        if !ui.get_has_document() || ui.get_home_session() || count!=run.home_before || ui.get_home_frame().size().width!=0 {
                            run.errors.push("home resources or updates survived the selected book".into());
                        }
                        finish(&ui,&state,&report,&scenario,&run);
                        if let Some(t)=stop.upgrade(){t.stop();}return;
                    }
                };
                if let Err(e)=action {run.errors.push(e.to_string());}
                run.home_before=count;run.step+=1;run.operations+=1;
                run.ready=Some(Instant::now()+Duration::from_millis(500));return;
            }
            if scenario == "page-input" {
                if page_input::step(&ui, &state, &mut run) {
                    finish(&ui, &state, &report, &scenario, &run);
                    if let Some(t) = stop.upgrade() { t.stop(); }
                } else {
                    run.step += 1;
                    run.operations += 1;
                    run.ready = Some(Instant::now() + Duration::from_millis(275));
                }
                return;
            }
            if scenario == "frontispiece" {
                use slint::platform::{PointerEventButton, WindowEvent};
                let w = ui.get_viewport_width();
                let h = ui.get_viewport_height();
                let inset = (w.min(h)/16.).clamp(24.,64.);
                let action = if ui.get_home_variant() == 0 {
                    slint::LogicalPosition::new((w*0.066).clamp(28.,108.)+35., h*0.895)
                } else {
                    slint::LogicalPosition::new(inset+35., h-inset-19.)
                };
                if let Some(folder) = &screenshots {
                    let name = match run.step {
                        4 => "action-hover.png".to_owned(),
                        5 => "action-pressed.png".to_owned(),
                        _ => format!("home-{}.png", run.step),
                    };
                    if let Err(e) = cover_snapshot(&ui, &folder.join(name)) { run.errors.push(e.to_string()); }
                }
                match run.step {
                    0 => ui.window().set_size(slint::LogicalSize::new(640.0, 480.0)),
                    1 => ui.invoke_theme_changed(2),
                    2 => ui.window().set_size(slint::LogicalSize::new(1280.0, 800.0)),
                    3 => ui.window().dispatch_event(WindowEvent::PointerMoved {position:action}),
                    4 => ui.window().dispatch_event(WindowEvent::PointerPressed {position:action,button:PointerEventButton::Left}),
                    _ => {
                        finish(&ui, &state, &report, &scenario, &run);
                        if let Some(timer) = stop.upgrade() { timer.stop(); }
                        return;
                    }
                }
                run.step += 1;
                run.operations += 1;
                run.ready = Some(Instant::now() + Duration::from_millis(180));
                return;
            }
            if scenario == "surfaces" {
                let step = run.step;
                let (maps,faces,map_us,face_us) = crate::glass::COST.with(|p|p.get());
                run.snapshots.push(json!({"step":step,"maps":maps,"faces":faces,"map_us":map_us,"face_us":face_us}));
                let names=["initial","bare","rail","search","contents","settings","dock","dock-open","night","settled","hidden","rested","original","inverted","restored"];
                if let Some(dir)=&screenshots {
                    if let Err(e)=snapshot(&ui,&dir.join(format!("{}.png",names[step as usize]))) {run.errors.push(e.to_string());}
                }
                match step {
                    0=>{ui.invoke_typography("fit-width".into(),0.); ui.set_show_toolbar(false);ui.invoke_edge_hover(240.,240.);}
                    1=>{ui.set_show_toolbar(true);}
                    2=>ui.set_show_search(true),
                    3=>{ui.set_show_search(false);ui.set_show_contents(true);}
                    4=>{ui.set_show_contents(false);ui.set_show_settings(true);}
                    5=>{ui.set_show_settings(false);ui.set_show_toolbar(false);ui.invoke_edge_hover(300.,ui.get_viewport_height()-6.);}
                    6=>ui.invoke_edge_hover(ui.get_dock_right()-42.,ui.get_dock_top()+20.),
                    7=>{ui.invoke_theme_changed(2);ui.set_show_settings(true);}
                    8=>{}
                    9=>{ui.invoke_dismiss_panels();ui.set_show_toolbar(false);ui.invoke_edge_hover(240.,240.);}
                    10=>{}
                    11=>{ui.invoke_theme_changed(0);ui.set_show_toolbar(true);ui.set_show_contents(true);}
                    12=>ui.invoke_page_color_changed(2),
                    13=>ui.invoke_page_color_changed(0),
                    _=>{finish(&ui,&state,&report,&scenario,&run); if let Some(t)=stop.upgrade(){t.stop();}return;}
                }
                run.step+=1;run.operations+=1;run.ready=Some(Instant::now()+Duration::from_millis(400));return;
            }
            if scenario == "glass" {
                let step = run.step;
                let center = ui.get_mode_center_x();
                let page = state.lock().unwrap().frame.as_ref().map(|f| f.page);
                let (calls, micros) = crate::glass::PROFILE.with(|p| p.get());
                let mut sample = json!({"step":step,"page":page,"radius":ui.get_header_height_logical()+ui.get_mode_depth(),
                    "center":center,"builds":calls,"total_us":micros,"visible":ui.get_top_visible()});
                let name = match step { 1 => "bare", 2 => "rest", 3 => "raised", 4 => "next",
                    5 => "night", 6 => "settled", 7 => "night-bare", 8 => "dial-warm-rest",
                    9 => "dial-warm-open", 10 => "dial-cool-open", 11 => "dial-cool-rest",
                    12 => "dial-night-rest", 13 => "dial-night-open", 14 => "dial-settled",
                    15 => "dial-preview", _ => "" };
                if let Some(dir) = &screenshots && !name.is_empty() {
                    if let Err(e) = snapshot(&ui,&dir.join(format!("{name}.png"))) { run.errors.push(e.to_string()); }
                }
                if step >= 8 {
                    let (maps, faces, _, _) = crate::glass::COST.with(|counter| counter.get());
                    sample["dial"] = json!({"name":name,"center":[ui.get_knob_center_x(),ui.get_knob_center_y()],
                        "readout":[ui.get_readout_x(),ui.get_readout_y()],
                        "radius":ui.get_knob_radius(),"visible":ui.get_bottom_visible(),"zoom":ui.get_pdf_zoom(),
                        "maps":maps,"faces":faces});
                }
                run.snapshots.push(sample);
                match step {
                    0 => {
                        ui.invoke_typography("fit-width".into(),0.);
                        ui.set_show_toolbar(false);
                        ui.invoke_edge_hover(260.,220.);
                    }
                    1 => ui.invoke_edge_hover(270.,14.),
                    2 => ui.invoke_edge_hover(center,24.),
                    3 => ui.invoke_next_page(1),
                    4 => ui.invoke_theme_changed(2),
                    5 => {}
                    6 => ui.invoke_edge_hover(260.,220.),
                    7 => {
                        ui.invoke_theme_changed(0);
                        ui.invoke_edge_hover(300., ui.get_viewport_height() - 6.);
                    }
                    8 | 12 => ui.invoke_edge_hover(ui.get_readout_x(), ui.get_readout_y()),
                    9 => ui.invoke_next_page(-1),
                    10 => ui.invoke_edge_hover(300., ui.get_viewport_height() - 6.),
                    11 => ui.invoke_theme_changed(2),
                    13 => {}
                    // Move only the needle and percentage, leaving document pixels fixed.
                    14 => ui.set_pdf_zoom(ui.get_pdf_zoom() * 1.31),
                    _ => {
                        finish(&ui,&state,&report,&scenario,&run);
                        if let Some(t)=stop.upgrade(){t.stop();}
                        return;
                    }
                }
                run.step+=1; run.operations+=1;
                run.ready=Some(Instant::now()+Duration::from_millis(if matches!(step,5|13) {850} else {280}));
                return;
            }
            if scenario == "instruments" {
                use slint::platform::{Key, PointerEventButton, WindowEvent};
                let pos=slint::LogicalPosition::new;
                let mode=pos(ui.get_mode_center_x(),ui.get_header_height_logical()+6.0);
                let move_to=|position: slint::LogicalPosition| { ui.invoke_edge_hover(position.x,position.y); ui.window().dispatch_event(WindowEvent::PointerMoved{position}); };
                let press=|position| ui.window().dispatch_event(WindowEvent::PointerPressed{position,button:PointerEventButton::Left});
                let release=|position| ui.window().dispatch_event(WindowEvent::PointerReleased{position,button:PointerEventButton::Left});
                let scroll=|delta: f32| {
                    move_to(mode);
                    for _ in 0..2 { ui.window().dispatch_event(WindowEvent::PointerScrolled{position:mode,delta_x:0.0,delta_y:delta.signum()*60.0}); }
                };
                let key=|text:slint::SharedString| {
                    ui.window().dispatch_event(WindowEvent::KeyPressed{text:text.clone()});
                    ui.window().dispatch_event(WindowEvent::KeyReleased{text});
                };
                let readout=pos(ui.get_readout_x(),ui.get_readout_y());
                let edge=pos(readout.x,ui.get_viewport_height()-0.5);
                let step=run.step;
                let s=state.lock().unwrap();
                let generation=s.generation;
                let page=s.frame.as_ref().map_or(0,|f| f.page);
                drop(s);
                if run.waiting {
                    run.operations+=1;
                    let count=ui.get_visible_count();
                    let wanted=match step {2|3|11=>Some(1),4|10=>Some(2),6|7|8|9=>Some(8),13=>Some(3),14=>Some(4),15=>Some(6),16|17=>Some(8),_=>None};
                    if wanted.is_some_and(|v|v!=count) { run.errors.push(format!("instrument {step}: expected {wanted:?} slots, got {count}")); }
                    if matches!(step,3|7|9|12|18) && run.panel_generation!=Some(generation) {
                        run.errors.push(format!("instrument {step} submitted extra document work"));
                    }
                    if step==8 && page!=8 {run.errors.push("Right did not turn all eight slots".into());}
                    if step==17 && (page!=24 || ui.get_pages().row_count()!=6) {run.errors.push("incomplete final eight-page spread".into());}
                    if step==20 && (ui.get_pdf_zoom()-0.25).abs()>0.001 {run.errors.push(format!("held monitor edge stopped at {}",ui.get_pdf_zoom()));}
                    if step==21 && !ui.get_zoom_pinned() {run.errors.push("releasing edge grip lost the axle".into());}
                    run.snapshots.push(json!({"step":step,"slots":ui.get_pages().row_count(),"stride":count,"rows":ui.get_grid_rows(),"page":page,"generation":generation,"zoom":ui.get_pdf_zoom(),"radius":ui.get_knob_radius(),
                        "mode_x":ui.get_mode_center_x(),"mode_radius":ui.get_header_height_logical()+ui.get_mode_depth()}));
                    if matches!(step,4|6|8|10|15|17|19|22) && let Some(folder)=&screenshots {
                        if let Err(e)=snapshot(&ui,&folder.join(format!("instrument-{step:02}.png"))) {run.errors.push(e.to_string());}
                    }
                }
                match step {
                    0=>{ui.invoke_fullscreen();ui.set_show_toolbar(true);},
                    1=>{
                        let track=ui.get_mode_label_radius();
                        let theta=48f32.to_radians();
                        let previous=pos(mode.x-track*theta.sin(),track*theta.cos());
                        move_to(previous);press(previous);release(previous);
                    },
                    2=>{run.panel_generation=Some(generation);scroll(120.0);},
                    3=>scroll(-120.0),
                    4=>{press(mode);move_to(pos(mode.x-76.0,mode.y));},
                    5=>release(pos(mode.x-76.0,mode.y)),
                    6=>{run.panel_generation=Some(generation);scroll(-120.0);},
                    7=>key(Key::RightArrow.into()),
                    8=>{run.panel_generation=Some(generation);scroll(-120.0);},
                    9=>scroll(120.0),
                    10=>scroll(120.0),
                    11=>{run.panel_generation=Some(generation);scroll(120.0);},
                    12..=15=>{
                        let grid=[1,2,4,5][step-12];
                        if step==12 {ui.set_pdf_multi(true);ui.invoke_multi_mode();}
                        ui.set_grid_choice(grid);ui.invoke_grid_changed(grid);
                    },
                    16=>{ui.invoke_jump_page("30".into());},
                    17=>{run.panel_generation=Some(generation);ui.invoke_edge_hover(readout.x,readout.y);},
                    18=>ui.invoke_typography("fit-page".into(),0.0),
                    19=>{press(readout);move_to(pos(readout.x,readout.y+8.0));move_to(edge);},
                    20=>release(edge),
                    21=>{let p=pos(ui.get_preset_page_x(),ui.get_preset_page_y());press(p);release(p);},
                    _=>{finish(&ui,&state,&report,&scenario,&run);if let Some(t)=stop.upgrade(){t.stop();}return;}
                }
                run.step+=1;run.waiting=true;
                run.ready=Some(Instant::now()+Duration::from_millis(if step==19 {1600}else{220}));return;
            }
            if scenario == "multipage" {
                use slint::platform::{Key, PointerEventButton, WindowEvent};
                let click = |x,y| {
                    let position=slint::LogicalPosition::new(x,y);
                    ui.window().dispatch_event(WindowEvent::PointerPressed {position,button:PointerEventButton::Left});
                    ui.window().dispatch_event(WindowEvent::PointerReleased {position,button:PointerEventButton::Left});
                };
                let page=state.lock().unwrap().frame.as_ref().map_or(-1,|f|f.page);
                let width=ui.get_viewport_width(); let height=ui.get_viewport_height();
                if run.waiting {
                    run.waiting=false;run.operations+=1;
                    let step=run.step;
                    if matches!(step,3|5|7) {
                        if ui.get_show_settings() || ui.get_show_contents() || ui.get_show_search() {run.errors.push(format!("outside click left a panel open at {step}"));}
                        if state.lock().unwrap().generation != run.panel_generation.unwrap() {run.errors.push("outside click invoked document work".into());}
                        if page != 0 {run.errors.push("outside click also turned a page".into());}
                    }
                    let expected=match step {8=>Some(4),9=>Some(8),10=>Some(4),11=>Some(0),_=>None};
                    if expected.is_some_and(|p|page!=p){run.errors.push(format!("grid navigation {step}: {page}"));}
                    if matches!(step,1|8|9|10|11|12|13|14|18) && ui.get_visible_count()!=4 {run.errors.push(format!("expected four-page grid at {step}"));}
                    if matches!(step,15) && ui.get_visible_count()!=2 {run.errors.push("dual fallback failed".into());}
                    if matches!(step,16|17) && ui.get_visible_count()!=1 {run.errors.push("single/narrow fallback failed".into());}
                    if step==11 && ui.get_pages().row_data(3).is_none_or(|t| t.matches.row_count()==0) {run.errors.push("fourth-page search highlight missing".into());}
                    run.snapshots.push(json!({"step":step,"page":page,"rows":ui.get_grid_rows(),"slots":ui.get_pages().row_count(),"generation":state.lock().unwrap().generation}));
                    if let Some(folder)=&screenshots { if matches!(step,1|2|3|12|15|17|18) {let _=snapshot(&ui,&folder.join(format!("grid-{step:02}.png")));} }
                }
                match run.step {
                    0=>{ui.window().set_size(slint::LogicalSize::new(1280.0,920.0));ui.set_pdf_multi(true);ui.invoke_multi_mode();},
                    1=>{ui.set_show_settings(true);run.panel_generation=Some(state.lock().unwrap().generation);},
                    2=>click(160.0,180.0),
                    3=>{ui.set_show_contents(true);run.panel_generation=Some(state.lock().unwrap().generation);},
                    4=>click(4.0,height*0.7),
                    5=>{ui.set_show_search(true);run.panel_generation=Some(state.lock().unwrap().generation);},
                    6=>click(width*0.6,height*0.7),
                    7=>ui.invoke_next_page(1),
                    8=>ui.invoke_next_page(1),
                    9=>{ui.window().dispatch_event(WindowEvent::KeyPressed{text:Key::LeftArrow.into()});ui.window().dispatch_event(WindowEvent::KeyReleased{text:Key::LeftArrow.into()});},
                    10=>ui.invoke_search("PAGE-04".into()),
                    11=>{ui.invoke_search("".into());ui.invoke_typography("zoom".into(),2.0);},
                    12=>ui.invoke_pan(120.0,120.0),
                    13=>{ui.invoke_typography("fit-page".into(),1.0);},
                    14=>{ui.set_double_page(true);ui.invoke_mode_changed(true);},
                    15=>{ui.set_double_page(false);ui.invoke_mode_changed(false);},
                    16=>{ui.window().set_size(slint::LogicalSize::new(640.0,480.0));ui.set_pdf_multi(true);ui.invoke_multi_mode();},
                    17=>{ui.window().set_size(slint::LogicalSize::new(1280.0,920.0));ui.set_show_toolbar(true);},
                    _=>{run.end=Some(counters());finish(&ui,&state,&report,&scenario,&run);if let Some(t)=stop.upgrade(){t.stop();}return;}
                }
                run.step+=1;run.waiting=true;run.ready=Some(Instant::now()+Duration::from_millis(220));return;
            }
            if scenario == "startup" {
                run.end = Some(counters());
                finish(&ui, &state, &report, &scenario, &run);
                if let Some(t) = stop.upgrade() {
                    t.stop();
                }
                return;
            }
            if scenario == "navigation" {
                use slint::platform::{Key, PointerEventButton, WindowEvent};
                let phase = run.step % 27;
                let book = run.step / 27;
                let page = state.lock().unwrap().frame.as_ref().map_or(-1, |f| f.page);
                if run.waiting {
                    run.waiting = false;
                    run.operations += 1;
                    let step=run.step;
                    if let Some(expected) = run.navigation_expected.take() {
                        if page != expected { run.errors.push(format!("keyboard navigation step {step}: expected page {expected}, got {page}")); }
                    }
                    if step % 27 == 16 && ui.get_show_contents() { run.errors.push("directory close control did not dismiss the panel".into()); }
                    run.snapshots.push(json!({"step":step,"page":page,"spread":ui.get_effective_spread(),"zoom":ui.get_pdf_zoom(),"reading_keys":ui.get_reading_keys(),"pannable":ui.get_pdf_pannable()}));
                }
                if run.step == 111 {
                    run.end=Some(counters()); finish(&ui,&state,&report,&scenario,&run);
                    if let Some(t)=stop.upgrade(){t.stop();} return;
                }
                let key = |text: slint::SharedString| {
                    ui.window().dispatch_event(WindowEvent::KeyPressed { text:text.clone() });
                    ui.window().dispatch_event(WindowEvent::KeyReleased { text });
                };
                let click = |x,y| {
                    let position=slint::LogicalPosition::new(x,y);
                    ui.invoke_edge_hover(x,y);
                    ui.window().dispatch_event(WindowEvent::PointerMoved {position});
                    ui.window().dispatch_event(WindowEvent::PointerPressed {position,button:PointerEventButton::Left});
                    ui.window().dispatch_event(WindowEvent::PointerReleased {position,button:PointerEventButton::Left});
                };
                let mode_center = ui.get_mode_center_x();
                let track = ui.get_mode_label_radius();
                let adjacent_mode = track * 48f32.to_radians().sin();
                let adjacent_y = track * 48f32.to_radians().cos();
                if run.step >= 108 {
                    match run.step {
                        108=>{ui.set_double_page(true);ui.invoke_mode_changed(true);ui.window().set_size(slint::LogicalSize::new(640.0,480.0));},
                        109=>{key(Key::RightArrow.into());run.navigation_expected=Some(1);},
                        110=>{key(Key::LeftArrow.into());run.navigation_expected=Some(0);},
                        _=>unreachable!(),
                    }
                    run.step+=1;run.ready=Some(Instant::now()+Duration::from_millis(200));run.waiting=true;return;
                }
                match phase {
                    0 => { ui.set_show_toolbar(true); if !ui.get_double_page() {click(mode_center+adjacent_mode,adjacent_y);} },
                    1|8|11|16|22 => { key(Key::RightArrow.into()); run.navigation_expected=Some(2); },
                    2|9|12|17|23 => { key(Key::LeftArrow.into()); run.navigation_expected=Some(0); },
                    3 => { key(Key::DownArrow.into()); run.navigation_expected=Some(2); },
                    4 => { key(Key::UpArrow.into()); run.navigation_expected=Some(0); },
                    5 => { key(Key::PageDown.into()); run.navigation_expected=Some(2); },
                    6 => { key(Key::PageUp.into()); run.navigation_expected=Some(0); },
                    7 => { if !ui.get_reflowable() { ui.set_bottom_hover(true); click(ui.get_dock_right()-40.0,ui.get_dock_top()+22.0); } else { click(mode_center,36.0); } },
                    10|13 => key(Key::F11.into()),
                    14 => ui.set_show_contents(true),
                    15 => click(340.0,ui.get_panel_top()+26.0),
                    18 => ui.invoke_focus_search(),
                    19 => key("ab".into()),
                    20 => { key(Key::LeftArrow.into()); run.navigation_expected=Some(page); },
                    21 => key(Key::Escape.into()),
                    24 => { ui.set_show_toolbar(true); click(mode_center-adjacent_mode,adjacent_y); },
                    25 => { key(Key::RightArrow.into()); run.navigation_expected=Some(1); },
                    26 => { key(Key::LeftArrow.into()); run.navigation_expected=Some(0); },
                    _ => unreachable!(),
                }
                // Repeat the same native key/focus path for fixed PDF, EPUB,
                // converted KF8 and native FB2. These original books exceed
                // three pages at the default viewport.
                if phase == 0 && book > 0 {
                    let paths=["tests/fixtures/core.pdf","tests/fixtures/core-epub3.epub","tests/fixtures/kindle-minimal.azw3","tests/fixtures/reading.fb2"];
                    open(PathBuf::from(paths[book]),&ui,&state,&mailbox);
                }
                run.step+=1;
                run.ready=Some(Instant::now()+Duration::from_millis(200)); run.waiting=true;
                return;
            }
            if scenario == "contents" {
                use slint::platform::WindowEvent;
                if run.waiting {
                    run.waiting=false; run.operations+=1;
                    if !ui.get_show_contents() { run.errors.push("contents overlay unexpectedly closed".into()); }
                    let step=run.step;
                    run.snapshots.push(json!({"step":step,"viewport":[ui.get_viewport_width(),ui.get_viewport_height()],"rows":ui.get_contents().row_count()}));
                    if let Some(folder)=&screenshots {
                        if let Err(e)=snapshot(&ui,&folder.join(format!("contents-{step:02}.png"))){run.errors.push(e.to_string());}
                    }
                }
                let scroll=|delta_y| ui.window().dispatch_event(WindowEvent::PointerScrolled {position:slint::LogicalPosition::new(160.0,260.0),delta_x:0.0,delta_y});
                match run.step {
                    0=>{ui.set_show_contents(true);ui.set_show_toolbar(true);},
                    1=>scroll(-400.0),
                    2=>scroll(-10000.0),
                    3=>ui.invoke_theme_changed(2),
                    4=>scroll(10000.0),
                    5=>ui.window().set_size(slint::LogicalSize::new(640.0,480.0)),
                    6=>scroll(-10000.0),
                    _=>{run.end=Some(counters());finish(&ui,&state,&report,&scenario,&run);if let Some(t)=stop.upgrade(){t.stop();}return;}
                }
                run.step+=1;run.waiting=true;run.ready=Some(Instant::now()+Duration::from_millis(350));
                return;
            }
            if scenario == "detents" {
                if run.waiting {
                    run.waiting = false;
                    run.operations += 1;
                    let value = ui.get_pdf_zoom();
                    let stop = ui.get_pdf_zoom_stop();
                    let step = run.step;
                    let spread = ui.get_effective_spread();
                    if (3..=25).contains(&step) || (28..=50).contains(&step) {
                        if run.dial_last.is_some_and(|last| value < last - 0.001) {
                            run.errors.push("positive dial input moved zoom backwards".into());
                        }
                    }
                    if (51..=73).contains(&step) && run.dial_last.is_some_and(|last| value > last + 0.001) {
                        run.errors.push("negative dial input moved zoom forwards".into());
                    }
                    run.dial_last = Some(value);
                    let s = state.lock().unwrap();
                    if let Some(frame) = &s.frame {
                        let column = if spread { ui.get_viewport_width() * 0.5 } else { ui.get_viewport_width() };
                        if stop == 1 && (frame.overflow.0 > 2.0 || frame.overflow.1 > 2.0) {
                            run.errors.push("fit-page detent cropped a page".into());
                        }
                        if stop == 2 && (frame.display_width < column - 2.0 || frame.display_height < ui.get_viewport_height() - 2.0) {
                            run.errors.push("fill detent did not fill the current page column".into());
                        }
                    }
                    drop(s);
                    if stop == 1 || stop == 2 {
                        run.dial_stops[usize::from(spread)*2 + (stop as usize-1)] += 1;
                    }
                    run.snapshots.push(json!({"step":step,"zoom":value,"stop":stop,"spread":spread,
                        "fill_ratio":ui.get_pdf_fill_zoom(),"width_ratio":ui.get_pdf_fit_width_zoom()}));
                    // Sample the actual axle after the lift has settled.
                    // It excludes the percent text and the document canvas.
                    if stop == 0 && !spread {
                        if let Ok(pixels) = ui.window().take_snapshot() {
                            let dpi = ui.window().scale_factor();
                            let cx = ui.get_knob_center_x()*dpi;
                            let cy = ui.get_knob_center_y()*dpi;
                            let radius = ui.get_knob_radius() * dpi;
                            let half_height = radius;
                            let mut hash = blake3::Hasher::new();
                            let bytes = pixels.as_bytes();
                            for y in (cy-half_height).max(0.0) as u32..((cy+half_height) as u32).min(pixels.height()) {
                                for x in (cx-radius).max(0.0) as u32..((cx+radius) as u32).min(pixels.width()) {
                                    let distance = ((x as f32 - cx)/radius).powi(2)
                                        + ((y as f32 - cy)/half_height).powi(2);
                                    if (0.49..0.85).contains(&distance) {
                                        let offset = ((y*pixels.width() + x)*4) as usize;
                                        hash.update(&bytes[offset..offset+4]);
                                    }
                                }
                            }
                            let hash = hash.finalize();
                            if let Some((previous, before)) = run.dial_signature {
                                if (1.12..1.8).contains(&(value / previous)) {
                                    run.dial_rotation_samples += 1;
                                    if before == hash { run.errors.push("dial index pixels did not rotate".into()); }
                                }
                            } else { run.dial_signature = Some((value, hash)); }
                        }
                    }
                    if matches!(step,75|76|78|80) {
                        if (value-2.0).abs()>0.01 || ui.get_zoom_dragging() || ui.get_controls_busy() {
                            run.errors.push(format!("dial travel or release failed at {step}: {value}"));
                        }
                        let s=state.lock().unwrap();
                        if let Some(f)=&s.frame {
                            let w=ui.get_viewport_width()/if spread{2.0}else{1.0};
                            let fit=(w/f.page_size.0).min(ui.get_viewport_height()/f.page_size.1);
                            let scale=f.scale / (fit * ui.window().scale_factor());
                            if (scale-value).abs()>0.01 || s.zoom_pending {
                                run.errors.push(format!("final zoom was not painted at {step}: {scale}"));
                            }
                        }
                    }
                    if step==76 && !ui.get_zoom_pinned() {
                        run.errors.push("a click on the compact dial did not open the presets".into());
                    }
                    if matches!(step,75|78|80|82|83|84|85|87) && !ui.get_show_zoom_pop() {
                        run.errors.push(format!("released gesture lost its controls at {step}"));
                    }
                    let expected = match step { 82|85|87 => Some(4.0), 83 => Some(1.0), 84 => Some(2.0), _ => None };
                    if expected.is_some_and(|target: f32| (target-value).abs()>0.012) {
                        run.errors.push(format!("unexpected circular or second drag scale at {step}: {value}"));
                    }
                    if step == 86 && (ui.get_show_zoom_pop() || ui.get_zoom_pinned() || ui.get_first_page() != 1) {
                        run.errors.push("outside click on pinned dial was not consumed".into());
                    }
                    if let Some(folder) = &screenshots {
                        if matches!(step,1|4|6|26|30|33|73|75|76|78|80) {
                            if let Err(e) = snapshot(&ui,&folder.join(format!("detent-{step:02}.png"))) { run.errors.push(e.to_string()); }
                        }
                    }
                }
                use slint::platform::{PointerEventButton, WindowEvent};
                let point = slint::LogicalPosition::new(ui.get_dock_right()-40.0,ui.get_dock_top()+22.0);
                let press=|position| ui.window().dispatch_event(WindowEvent::PointerPressed {position,button:PointerEventButton::Left});
                let release=|position| ui.window().dispatch_event(WindowEvent::PointerReleased {position,button:PointerEventButton::Left});
                let movement=|position: slint::LogicalPosition| {
                    ui.invoke_edge_hover(position.x,position.y);
                    ui.window().dispatch_event(WindowEvent::PointerMoved {position});
                };
                let circle = |center: slint::LogicalPosition, radius: f32, clockwise: bool| {
                    let at = |degrees: i32| {
                        let angle = (degrees as f32).to_radians();
                        slint::LogicalPosition::new(center.x+radius*angle.sin(),center.y-radius*angle.cos())
                    };
                    movement(at(0)); press(at(0));
                    let mut previous=ui.get_pdf_zoom();
                    let mut monotonic=true;
                    for step in (12..=360).step_by(12) {
                        movement(at(if clockwise {step} else {-step}));
                        let now=ui.get_pdf_zoom();
                        monotonic &= if clockwise {now+0.001>=previous} else {now<=previous+0.001};
                        previous=now;
                    }
                    release(at(0)); monotonic
                };
                match run.step {
                    0 => { ui.set_show_toolbar(true); ui.set_double_page(false); ui.invoke_mode_changed(false); ui.invoke_typography("fit-page".into(),0.0); },
                    1 | 26 => ui.invoke_typography("zoom".into(),0.8),
                    2..=24 | 27..=49 => ui.window().dispatch_event(WindowEvent::PointerScrolled {position:point,delta_x:0.0,delta_y:120.0}),
                    25 => { ui.set_double_page(true); ui.invoke_mode_changed(true); ui.invoke_typography("fit-page".into(),0.0); },
                    50..=72 => ui.window().dispatch_event(WindowEvent::PointerScrolled {position:point,delta_x:0.0,delta_y:-120.0}),
                    73 => ui.invoke_typography("fit-page".into(),0.0),
                    74 => {
                        let point=slint::LogicalPosition::new(ui.get_readout_x(),ui.get_readout_y());
                        let to=slint::LogicalPosition::new(point.x+144.0,point.y-9.0);
                        press(point);
                        movement(to);
                        release(to);
                    },
                    75 => {
                        let canvas=slint::LogicalPosition::new(ui.get_viewport_width()/2.0,ui.get_viewport_height()/2.0);
                        movement(canvas); press(canvas); release(canvas);
                        movement(point); press(point); release(point);
                    },
                    76 | 78 => ui.invoke_typography("fit-page".into(),0.0),
                    77 => {
                        let from=slint::LogicalPosition::new(ui.get_knob_center_x(),ui.get_knob_center_y());
                        press(from);
                        // The same path at a different event frequency, crossing the edge
                        // of the popover and reversing before returning to its end.
                        let mut previous=ui.get_pdf_zoom();
                        for distance in (4..=144).step_by(4) {
                            movement(slint::LogicalPosition::new(from.x+distance as f32,from.y-(distance%12) as f32/3.0));
                            let value=ui.get_pdf_zoom();
                            if value+0.001<previous {run.errors.push("zoom reversed during outward drag".into());}
                            previous=value;
                        }
                        movement(from);
                        if (ui.get_pdf_zoom()-1.0).abs()>0.001 {
                            run.errors.push("reversing the dial did not return to its initial scale".into());
                        }
                        let to=slint::LogicalPosition::new(from.x+144.0,from.y-1.0);
                        movement(to);
                        release(to);
                    },
                    79 => {
                        ui.set_show_toolbar(false);
                        ui.set_zoom_pinned(false);
                        ui.invoke_edge_hover(point.x,point.y);
                        let from=slint::LogicalPosition::new(ui.get_knob_center_x(),ui.get_knob_center_y());
                        press(from);
                        movement(slint::LogicalPosition::new(from.x+3.0,from.y-60.0));
                        release(slint::LogicalPosition::new(from.x+10.0,from.y-144.0));
                    },
                    80 => ui.invoke_typography("fit-page".into(),0.0),
                    81|82 => {
                        let center=slint::LogicalPosition::new(ui.get_knob_center_x(),ui.get_knob_center_y());
                        if !circle(center,34.0,run.step==81) {run.errors.push("circular input changed direction at a quadrant".into());}
                    },
                    83|84 => {
                        let center=slint::LogicalPosition::new(ui.get_knob_center_x(),ui.get_knob_center_y());
                        movement(center); press(center);
                        let outside=slint::LogicalPosition::new(center.x+2.0,center.y-144.0);
                        movement(outside); release(outside);
                        ui.invoke_edge_hover(-1.0,-1.0);
                    },
                    85 => {
                        let canvas=slint::LogicalPosition::new(ui.get_viewport_width()*0.75,ui.get_viewport_height()/2.0);
                        movement(canvas); press(canvas); release(canvas);
                    },
                    86 => {
                        ui.invoke_typography("fit-page".into(),0.0);
                        let center=slint::LogicalPosition::new(ui.get_knob_center_x(),ui.get_knob_center_y());
                        if !circle(center,ui.get_knob_radius()*0.92,true) {run.errors.push("compact rim changed direction".into());}
                    },
                    _ => {
                        if ui.get_pdf_zoom()<1.05 { run.errors.push("linear dial drag did not enlarge the page".into()); }
                        if run.dial_stops.contains(&0) { run.errors.push("page/fill detent was not reached in both single and double modes".into()); }
                        if run.dial_rotation_samples == 0 { run.errors.push("dial pixel rotation was not sampled".into()); }
                        run.end = Some(counters()); finish(&ui,&state,&report,&scenario,&run);
                        if let Some(t) = stop.upgrade() { t.stop(); } return;
                    }
                }
                run.step += 1; run.waiting = true;
                run.ready = Some(Instant::now()+Duration::from_millis(180)); return;
            }
            if scenario == "chrome" {
                let viewport=(ui.get_viewport_width(),ui.get_viewport_height());
                if run.waiting {
                    run.waiting=false; run.operations+=1;
                    if matches!(run.step,1|2|3) && run.chrome_viewport!=Some(viewport) {
                        run.errors.push("hover/pinned chrome changed document viewport".into());
                    }
                    if (run.step==1 && (!ui.get_top_visible() || ui.get_bottom_visible()))
                        || (run.step==2 && (ui.get_top_visible() || !ui.get_bottom_visible()))
                        || (run.step==3 && (!ui.get_top_visible() || !ui.get_bottom_visible())) {
                        run.errors.push("edge reveal did not independently show/hide its controls".into());
                    }
                    let (native_full, fills_monitor)=ui.window().with_winit_window(|w| {
                        let fills=w.current_monitor().is_some_and(|m| w.inner_size()==m.size() && w.inner_position().ok()==Some(m.position()));
                        (w.fullscreen().is_some(),fills)
                    }).unwrap_or_default();
                    if (run.step==4 && !native_full) || (run.step==5 && native_full) {
                        run.errors.push("native fullscreen state did not change".into());
                    }
                    if run.step==4 && !fills_monitor { run.errors.push("maximized reader left system chrome in the monitor bounds".into()); }
                    let (page,origin)={let s=state.lock().unwrap();(s.frame.as_ref().map_or(-1,|f|f.page),s.frame.as_ref().map_or(0.0,|f|f.origin.1))};
                    if (run.step==9 && ui.get_pdf_zoom()<1.1) || (run.step==10 && ui.get_pdf_zoom()<1.3) {
                        run.errors.push("zoom dial wheel/drag did not change zoom".into());
                    }
                    if run.step==8 && !ui.get_show_zoom_pop() {run.errors.push("hover did not unfold the zoom controls".into());}
                    if run.step==11 && (page!=0 || state.lock().unwrap().generation!=run.panel_generation.unwrap_or(0)) {
                        run.errors.push("progress drag rendered before release".into());
                    }
                    if run.step==12 && page<18 {run.errors.push("progress drag did not seek the page on release".into());}
                    if run.step==13 && origin<=run.pan_origin {run.errors.push("right-button page drag did not pan".into());}
                    if run.step==14 && !ui.get_page_entry_active() {run.errors.push("page number did not open its editor".into());}
                    if run.step==15 && page!=7 {run.errors.push("editable page number did not navigate to page 8".into());}
                    if run.step==16 && page!=8 {run.errors.push("Right key after magnifying did not turn the page".into());}
                    if run.step==17 && page!=7 {run.errors.push("Left key after magnifying did not turn the page".into());}
                    if run.step==18 && ui.get_pdf_zoom_stop()!=3 {run.errors.push("fit width preset was not selected".into());}
                    if run.step==19 && (ui.get_pdf_zoom()-1.0).abs()>0.01 {run.errors.push("100% preset did not restore fit page".into());}
                    if run.step==21 && (page!=7 || !ui.get_page_entry_active()) {run.errors.push("out-of-range page was not rejected".into());}
                    if run.step==22 && (page!=7 || ui.get_page_entry_active()) {run.errors.push("Escape did not cancel the page editor".into());}
                    if run.step==24 && !ui.get_show_zoom_pop() {run.errors.push("zoom controls did not fit in the narrow window".into());}
                    if run.step==26 && !ui.get_page_entry_active() {run.errors.push("narrow reflow footer did not open the page editor".into());}
                    let step=run.step;
                    run.snapshots.push(json!({"step":step,"viewport":[viewport.0,viewport.1],"native_fullscreen":native_full,
                        "fills_monitor":fills_monitor,"top_visible":ui.get_top_visible(),"bottom_visible":ui.get_bottom_visible(),"zoom":ui.get_pdf_zoom(),"page":page,"origin_y":origin}));
                    if let Some(folder)=&screenshots {
                        if let Err(e)=snapshot(&ui,&folder.join(format!("chrome-{step:02}.png"))) { run.errors.push(e.to_string()); }
                    }
                }
                use slint::platform::{Key,WindowEvent,PointerEventButton};
                let dock_left=ui.get_dock_left();
                let dock_right=ui.get_dock_right();
                let point=slint::LogicalPosition::new(dock_right-40.0,ui.get_dock_top()+22.0);
                let progress_start=slint::LogicalPosition::new(dock_left+130.0,point.y);
                let progress_end=slint::LogicalPosition::new(dock_right-132.0,point.y);
                let key=|text:slint::SharedString| {
                    ui.window().dispatch_event(WindowEvent::KeyPressed{text:text.clone()});
                    ui.window().dispatch_event(WindowEvent::KeyReleased{text});
                };
                let drag=|from:slint::LogicalPosition,to:slint::LogicalPosition,button| {
                    ui.window().dispatch_event(WindowEvent::PointerPressed{position:from,button});
                    ui.window().dispatch_event(WindowEvent::PointerMoved{position:to});
                    ui.window().dispatch_event(WindowEvent::PointerReleased{position:to,button});
                };
                match run.step {
                    0=>{run.chrome_viewport=Some(viewport);ui.invoke_edge_hover(viewport.0/2.0,40.0);},
                    1=>{ui.invoke_edge_hover(viewport.0/2.0,viewport.1-40.0);},
                    2=>{ui.invoke_edge_hover(viewport.0/2.0,viewport.1/2.0);ui.set_show_toolbar(true);},
                    3=>{ui.window().with_winit_window(|w|w.set_maximized(true));},
                    4=>ui.invoke_fullscreen(),
                    5=>{ui.set_show_toolbar(false);ui.set_top_hover(false);ui.set_bottom_hover(false);ui.set_show_settings(true);},
                    6=>{ui.set_show_settings(false);ui.set_show_toolbar(false);},
                    7=>{ui.invoke_edge_hover(point.x,viewport.1-4.0);},
                    8=>ui.window().dispatch_event(WindowEvent::PointerScrolled{position:point,delta_x:0.0,delta_y:120.0}),
                    9=>{
                        let dial=slint::LogicalPosition::new(ui.get_knob_center_x(),ui.get_knob_center_y());
                        drag(dial,slint::LogicalPosition::new(dial.x+128.0,dial.y+28.0),PointerEventButton::Left);
                    },
                    10=>{
                        run.panel_generation=Some(state.lock().unwrap().generation);
                        ui.invoke_edge_hover(progress_start.x,viewport.1-4.0);
                        ui.invoke_edge_hover(progress_start.x,progress_start.y);
                        ui.window().dispatch_event(WindowEvent::PointerPressed{position:progress_start,button:PointerEventButton::Left});
                        ui.window().dispatch_event(WindowEvent::PointerMoved{position:progress_end});
                    },
                    11=>ui.window().dispatch_event(WindowEvent::PointerReleased{position:progress_end,button:PointerEventButton::Left}),
                    12=>{
                        run.pan_origin=state.lock().unwrap().frame.as_ref().unwrap().origin.1;
                        let from=slint::LogicalPosition::new(viewport.0*0.7,viewport.1*0.5);
                        drag(from,slint::LogicalPosition::new(from.x-80.0,from.y-60.0),PointerEventButton::Right);
                    },
                    13=>{
                        let label=slint::LogicalPosition::new(dock_left+48.0,point.y);
                        ui.invoke_edge_hover(label.x,viewport.1-4.0);
                        drag(label,label,PointerEventButton::Left);
                    },
                    14=>{key("8".into()); key(Key::Return.into());},
                    15=>key(Key::RightArrow.into()),
                    16=>{key(Key::LeftArrow.into());ui.invoke_edge_hover(point.x,point.y);},
                    17=>{
                        ui.invoke_edge_hover(point.x,point.y);
                        let preset=slint::LogicalPosition::new(ui.get_preset_width_x(),ui.get_preset_width_y());
                        drag(preset,preset,PointerEventButton::Left);
                    },
                    18=>{
                        let preset=slint::LogicalPosition::new(ui.get_preset_page_x(),ui.get_preset_page_y());
                        drag(preset,preset,PointerEventButton::Left);
                    },
                    19=>{ui.set_zoom_hover(false);ui.invoke_focus_page_entry();},
                    20=>{key("0".into());key(Key::Return.into());},
                    21=>{key(Key::Escape.into());ui.set_bottom_hover(false);},
                    22=>{ui.invoke_theme_changed(2);ui.window().set_size(slint::LogicalSize::new(640.0,480.0));},
                    23=>{ui.invoke_edge_hover(point.x,point.y);},
                    24=>{ui.set_zoom_hover(false);open(PathBuf::from("tests/fixtures/core-epub3.epub"),&ui,&state,&mailbox);},
                    25=>{ui.invoke_focus_page_entry();},
                    26=>{key(Key::Escape.into());ui.set_bottom_hover(true);},
                    _=>{run.end=Some(counters());finish(&ui,&state,&report,&scenario,&run);if let Some(t)=stop.upgrade(){t.stop();}return;}
                }
                run.step+=1;run.waiting=true;run.ready=Some(Instant::now()+Duration::from_millis(400));return;
            }
            if scenario == "features" {
                if run.waiting {
                    run.waiting=false;run.operations+=1;
                    let step=run.step;
                    let s=state.lock().unwrap();
                    if matches!(step,1|2) && s.frame.as_ref().map(|f|f.page)!=Some(if step==1{1}else{0}) {run.errors.push(format!("page click failed at step {step}"));}
                    if step==11 && s.frame.as_ref().is_none_or(|f|f.page<10) {run.errors.push("whole-book PDF search did not navigate".into());}
                    if step==23 && ui.get_reflowable() {run.errors.push("fixed-layout EPUB was treated as reflowable".into());}
                    if let Some(frame)=&s.frame {
                        let record=json!({"step":step,"page":frame.page,"right":s.right.as_ref().map(|f|f.page),"overflow":[frame.overflow.0,frame.overflow.1],"pixels":[frame.width,frame.height],"glyphs":frame.glyphs.len(),"images":frame.images.len(),"cache_bytes":s.metrics.max_cache,"counters":counters()});
                        run.snapshots.push(record);
                    }
                    drop(s);
                    if let Some(folder)=&screenshots {
                        if let Err(error)=snapshot(&ui,&folder.join(format!("feature-{step:02}.png"))){run.errors.push(error.to_string());}
                    }
                }
                match run.step {
                    0=>{let s=state.lock().unwrap();let f=s.frame.as_ref().unwrap();let(x,y)=(f.display_width*0.9,f.display_height*0.5);drop(s);ui.invoke_hit(0,x,y);},
                    1=>ui.invoke_hit(0,5.0,20.0),
                    2=>ui.invoke_typography("zoom".into(),2.5),
                    3=>ui.invoke_pan(180.0,300.0),
                    4=>{ui.invoke_theme_changed(2);ui.invoke_page_color_changed(1);},
                    5=>ui.invoke_page_color_changed(2),
                    6=>{ui.invoke_theme_changed(0);ui.invoke_page_color_changed(0);ui.set_double_page(true);ui.invoke_mode_changed(true);},
                    7=>ui.invoke_typography("fit-width".into(),0.0),
                    8=>ui.invoke_typography("zoom".into(),2.0),
                    9=>ui.invoke_pan(160.0,300.0),
                    10=>{ui.set_show_search(true);ui.invoke_search("Paragraph 20-24".into());},
                    11=>{ui.set_show_search(false);ui.invoke_search("".into());ui.set_double_page(false);ui.invoke_mode_changed(false);open(PathBuf::from("tests/fixtures/reading.md"),&ui,&state,&mailbox);},
                    12=>ui.set_show_contents(true),
                    13=>{ui.set_show_contents(false);ui.invoke_typography("size".into(),24.0);},
                    14=>{ui.invoke_typography("size".into(),18.0);ui.invoke_next_page(1);},
                    15=>open(PathBuf::from("tests/fixtures/text-gb18030.txt"),&ui,&state,&mailbox),
                    16=>ui.invoke_encoding_changed("GB18030".into()),
                    17=>open(PathBuf::from("tests/fixtures/text-utf16le.txt"),&ui,&state,&mailbox),
                    18=>open(PathBuf::from("tests/fixtures/advanced.epub"),&ui,&state,&mailbox),
                    19=>ui.invoke_theme_changed(2),
                    20=>{ui.invoke_theme_changed(0);open(PathBuf::from("tests/fixtures/vertical.epub"),&ui,&state,&mailbox);},
                    21=>open(PathBuf::from("tests/fixtures/fixed.epub"),&ui,&state,&mailbox),
                    22=>ui.invoke_typography("zoom".into(),1.8),
                    _=>{run.end=Some(counters());finish(&ui,&state,&report,&scenario,&run);if let Some(t)=stop.upgrade(){t.stop();}return;}
                }
                run.step+=1;run.waiting=true;run.ready=Some(Instant::now()+Duration::from_millis(400));return;
            }
            if scenario == "stress" {
                if run.baseline.is_none() {
                    run.baseline = Some(counters());
                }
                if run.waiting {
                    run.waiting = false;
                    run.operations += 1;
                    if run.operations % 50 == 0 {
                        let c = counters();
                        let operation = run.operations;
                        run.snapshots
                            .push(json!({"operation":operation,"counters":c}));
                    }
                }
                if run.step < 500 {
                    let at_end = {
                        let s = state.lock().unwrap();
                        s.pagination_complete
                            && s.frame.as_ref().is_some_and(|f| f.page + 1 >= s.count)
                    };
                    if at_end {
                        submit(&state, &mailbox, |r| {
                            r.page = 0;
                            r.navigation = Navigation::Page(0);
                        });
                    } else {
                        ui.invoke_next_page(1);
                    }
                } else if run.step < 600 {
                    ui.invoke_typography("size".into(), 14.0 + (run.step % 12) as f32);
                } else if run.step < 700 {
                    let w = if run.step % 2 == 0 { 1040.0 } else { 1180.0 };
                    ui.window().set_size(slint::LogicalSize::new(w, 840.0));
                } else if run.step < 730 {
                    let path = PathBuf::from(if run.step % 2 == 0 {
                        "tests/fixtures/core.pdf"
                    } else {
                        "tests/fixtures/core-epub3.epub"
                    });
                    open(path, &ui, &state, &mailbox);
                } else {
                    run.end = Some(counters());
                    finish(&ui, &state, &report, &scenario, &run);
                    if let Some(t) = stop.upgrade() {
                        t.stop();
                    }
                    return;
                }
                run.step += 1;
                run.waiting = true;
                // Wait past the actual slider/resize debounce; the next tick then
                // checks the worker generation, not a guessed rendering duration.
                if run.step > 500 && run.step <= 700 {
                    run.ready = Some(Instant::now() + Duration::from_millis(190));
                }
                return;
            }
            if scenario == "visual" {
                if Instant::now() < run.ready.unwrap() {
                    return;
                }
                if run.waiting {
                    run.waiting = false;
                    if ui.get_reflowable() && run.step == 8 && ui.get_font_size_pt() <= 18.0 {
                        run.errors.push("typography slider did not apply the chosen font size".into());
                    }
                    if matches!(run.step,9|11) && !ui.get_choice_open() {
                        run.errors.push("font list did not open".into());
                    }
                    if matches!(run.step,10|12) && (ui.get_choice_open() || !ui.get_show_settings()) {
                        run.errors.push("font list did not close back to the settings panel".into());
                    }
                    if run.step==12 && ui.get_fonts().row_count()>1 && ui.get_family_label()=="默认衬线" {
                        run.errors.push("font choice was not committed".into());
                    }
                    if let Some(folder) = &screenshots {
                        let path = folder.join(format!("view-{:02}.png", run.step));
                        if let Err(e) = snapshot(&ui, &path) {
                            run.errors.push(e.to_string());
                        }
                    }
                }
                match run.step {
                    0 => ui.invoke_next_page(1),
                    1 => {
                        ui.set_double_page(true);
                        ui.invoke_mode_changed(true);
                    }
                    2 => {
                        ui.set_show_contents(true);
                    }
                    3 => {
                        ui.set_show_contents(false);
                        ui.set_show_settings(true);
                    }
                    4 => {
                        ui.set_show_settings(false);
                        ui.invoke_theme_changed(2);
                        ui.set_show_contents(true);
                    }
                    5 => {
                        ui.set_show_contents(false);
                        ui.invoke_theme_changed(0);
                        ui.window().set_size(slint::LogicalSize::new(680.0, 760.0));
                    }
                    6 => {
                        ui.window().set_size(slint::LogicalSize::new(640.0, 480.0));
                        ui.set_show_settings(true);
                    }
                    7|8|10 if ui.get_reflowable() => {
                        use slint::platform::{PointerEventButton, WindowEvent};
                        let position =
                            slint::LogicalPosition::new(ui.get_viewport_width() - 155.0, ui.get_panel_top() + if run.step==7 {216.0} else {180.0});
                        ui.window().dispatch_event(WindowEvent::PointerPressed { position, button:PointerEventButton::Left });
                        ui.window().dispatch_event(WindowEvent::PointerReleased { position, button:PointerEventButton::Left });
                    }
                    9|11 if ui.get_reflowable() => {
                        use slint::platform::{Key,WindowEvent};
                        let key=|text: slint::SharedString| {
                            ui.window().dispatch_event(WindowEvent::KeyPressed{text:text.clone()});
                            ui.window().dispatch_event(WindowEvent::KeyReleased{text});
                        };
                        if run.step==9 {key(Key::Escape.into());} else {key(Key::DownArrow.into());key(Key::Return.into());}
                    }
                    _ => {
                        finish(&ui, &state, &report, &scenario, &run);
                        if let Some(t) = stop.upgrade() {
                            t.stop();
                        }
                        return;
                    }
                }
                run.step += 1;
                run.waiting = true;
                run.ready = Some(Instant::now() + Duration::from_millis(400));
                return;
            }
            let elapsed = run.ready.unwrap().elapsed();
            if elapsed >= Duration::from_secs(2) && run.idle_start.is_none() {
                run.idle_start = Some(counters());
                run.idle_at = Some(Instant::now());
            }
            if elapsed >= Duration::from_secs(7) {
                run.end = Some(counters());
                finish(&ui, &state, &report, &scenario, &run);
                if let Some(t) = stop.upgrade() {
                    t.stop();
                }
            }
        },
    );
    Some(timer)
}
fn finish(ui: &ReaderWindow, state: &Arc<Mutex<State>>, report: &Path, scenario: &str, run: &Run) {
    let s = state.lock().unwrap();
    let idle_ms = run.idle_at.map(|t| t.elapsed().as_secs_f64() * 1000.0);
    let cpu_delta =
        run.end.as_ref().zip(run.idle_start.as_ref()).map(|(a, b)| {
            a["cpu_ms"].as_f64().unwrap_or(0.0) - b["cpu_ms"].as_f64().unwrap_or(0.0)
        });
    let document = s.frame.is_some();
    let report_value = json!({"schema":1,"instrumentation":"qa feature; 25ms wakeup during QA only; CPU includes probe overhead",
        "scenario":scenario,"document":document,"first_frame_assigned_ms":s.metrics.first_frame_ms,
        "native_snapshot_ready_ms":run.painted_ms,"first_counters":run.first_counters,
        "idle_start":run.idle_start,"end":run.end,"idle_wall_ms":idle_ms,"idle_cpu_ms":cpu_delta,
        "max_cache_bytes":s.metrics.max_cache,"frames":s.metrics.frames,"operations":run.operations,
        "baseline":run.baseline,"samples":run.snapshots,"errors":run.errors,
        "viewport":[ui.get_viewport_width(),ui.get_viewport_height()],"dpi":ui.window().scale_factor(),
        "spread":ui.get_effective_spread(),"page":s.frame.as_ref().map(|f|f.page),"right_page":s.right.as_ref().map(|f|f.page),"count":s.count});
    if let Some(folder) = report.parent() {
        let _ = fs::create_dir_all(folder);
    }
    let _ = fs::write(report, serde_json::to_vec_pretty(&report_value).unwrap());
    drop(s);
    let _ = slint::quit_event_loop();
}
