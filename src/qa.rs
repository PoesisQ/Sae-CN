//! Explicit, optional native-window diagnostics. Never enabled by a release build.
//! Reports contain timings and counters, never book paths, titles or text.
use super::*;
use serde_json::{Value, json};
use slint::Model;
use std::{cell::RefCell, fs, io::BufWriter, path::Path};

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
            "right_page":u.right.as_ref().map(|f|f.page),"cache_bytes":u.cache_bytes,
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
    let pixels = ui.window().take_snapshot()?;
    let file = BufWriter::new(fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()?
        .write_image_data(pixels.as_bytes())?;
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
                        || s.frame.is_some() && s.generation == s.completed_generation,
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
                    run.snapshots.push(json!({"step":step,"page":page,"spread":ui.get_effective_spread(),"zoom":ui.get_pdf_zoom()}));
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
                    ui.window().dispatch_event(WindowEvent::PointerPressed {position,button:PointerEventButton::Left});
                    ui.window().dispatch_event(WindowEvent::PointerReleased {position,button:PointerEventButton::Left});
                };
                let width=ui.get_viewport_width()+32.0;
                let height=ui.get_viewport_height();
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
                    0 => { ui.set_show_toolbar(true); click(width-148.0,38.0); },
                    1|8|11|16|22 => { key(Key::RightArrow.into()); run.navigation_expected=Some(2); },
                    2|9|12|17|23 => { key(Key::LeftArrow.into()); run.navigation_expected=Some(0); },
                    3 => { key(Key::DownArrow.into()); run.navigation_expected=Some(2); },
                    4 => { key(Key::UpArrow.into()); run.navigation_expected=Some(0); },
                    5 => { key(Key::PageDown.into()); run.navigation_expected=Some(2); },
                    6 => { key(Key::PageUp.into()); run.navigation_expected=Some(0); },
                    7 => { if !ui.get_reflowable() { ui.set_bottom_hover(true); click(width*0.5+171.0,height-43.0); } else { click(width-148.0,38.0); } },
                    10|13 => key(Key::F11.into()),
                    14 => ui.set_show_contents(true),
                    15 => click((width*0.28).clamp(280.0,340.0)-22.0,104.0),
                    18 => ui.invoke_focus_search(),
                    19 => key("ab".into()),
                    20 => { key(Key::LeftArrow.into()); run.navigation_expected=Some(page); },
                    21 => key(Key::Escape.into()),
                    24 => { ui.set_show_toolbar(true); click(width-192.0,38.0); },
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
                        let column = if spread { (ui.get_viewport_width() - 32.0) * 0.5 } else { ui.get_viewport_width() };
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
                    // The ROI is the 24px dial, inside the fixed 560px footer.
                    // It excludes the percent text and the document canvas.
                    if stop == 0 && !spread {
                        if let Ok(pixels) = ui.window().take_snapshot() {
                            let dpi = ui.window().scale_factor();
                            let cx = ((ui.get_viewport_width()+32.0)*0.5+132.0)*dpi;
                            let cy = (ui.get_viewport_height()-43.0)*dpi;
                            let mut hash = blake3::Hasher::new();
                            for y in ((cy-13.0*dpi).max(0.0) as u32)..((cy+13.0*dpi) as u32).min(pixels.height()) {
                                let a = ((y*pixels.width() + (cx-13.0*dpi).max(0.0) as u32)*4) as usize;
                                let b = ((y*pixels.width() + ((cx+13.0*dpi) as u32).min(pixels.width()))*4) as usize;
                                hash.update(&pixels.as_bytes()[a..b]);
                            }
                            let hash = hash.finalize();
                            if let Some((previous, before)) = run.dial_signature {
                                if (1.12..1.8).contains(&(value / previous)) {
                                    run.dial_rotation_samples += 1;
                                    if before == hash { run.errors.push("dial needle pixels did not rotate".into()); }
                                }
                            } else { run.dial_signature = Some((value, hash)); }
                        }
                    }
                    if let Some(folder) = &screenshots {
                        if matches!(step,1|4|6|26|30|33|73|75) {
                            if let Err(e) = snapshot(&ui,&folder.join(format!("detent-{step:02}.png"))) { run.errors.push(e.to_string()); }
                        }
                    }
                }
                use slint::platform::WindowEvent;
                let point = slint::LogicalPosition::new((ui.get_viewport_width()+32.0)*0.5+132.0,ui.get_viewport_height()-43.0);
                match run.step {
                    0 => { ui.set_show_toolbar(true); ui.set_double_page(false); ui.invoke_mode_changed(false); ui.invoke_typography("fit-page".into(),0.0); },
                    1 | 26 => ui.invoke_typography("zoom".into(),0.8),
                    2..=24 | 27..=49 => ui.window().dispatch_event(WindowEvent::PointerScrolled {position:point,delta_x:0.0,delta_y:120.0}),
                    25 => { ui.set_double_page(true); ui.invoke_mode_changed(true); ui.invoke_typography("fit-page".into(),0.0); },
                    50..=72 => ui.window().dispatch_event(WindowEvent::PointerScrolled {position:point,delta_x:0.0,delta_y:-120.0}),
                    73 => ui.invoke_typography("fit-page".into(),0.0),
                    74 => {
                        use slint::platform::PointerEventButton;
                        let from=slint::LogicalPosition::new(point.x+9.0,point.y);
                        let to=slint::LogicalPosition::new(point.x,point.y+9.0);
                        ui.window().dispatch_event(WindowEvent::PointerPressed {position:from,button:PointerEventButton::Left});
                        ui.window().dispatch_event(WindowEvent::PointerMoved {position:to});
                        ui.window().dispatch_event(WindowEvent::PointerReleased {position:to,button:PointerEventButton::Left});
                    },
                    _ => {
                        if ui.get_pdf_zoom()<1.05 || ui.get_pdf_zoom_stop()!=2 { run.errors.push("circular dial drag did not turn clockwise into the fill stop".into()); }
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
                    let native_full=ui.window().with_winit_window(|w| w.fullscreen().is_some()).unwrap_or(false);
                    if (run.step==4 && !native_full) || (run.step==5 && native_full) {
                        run.errors.push("native fullscreen state did not change".into());
                    }
                    let (page,origin)={let s=state.lock().unwrap();(s.frame.as_ref().map_or(-1,|f|f.page),s.frame.as_ref().map_or(0.0,|f|f.origin.1))};
                    if (run.step==9 && ui.get_pdf_zoom()<1.1) || (run.step==10 && ui.get_pdf_zoom()<1.3) {
                        run.errors.push("zoom dial wheel/drag did not change zoom".into());
                    }
                    if run.step==11 && page<18 {run.errors.push("progress drag did not seek the page".into());}
                    if run.step==12 && origin<=run.pan_origin {run.errors.push("right-button page drag did not pan".into());}
                    if run.step==13 && (ui.get_pdf_zoom()-1.0).abs()>0.01 {run.errors.push("dial click did not restore fit page".into());}
                    let step=run.step;
                    run.snapshots.push(json!({"step":step,"viewport":[viewport.0,viewport.1],"native_fullscreen":native_full,
                        "top_visible":ui.get_top_visible(),"bottom_visible":ui.get_bottom_visible(),"zoom":ui.get_pdf_zoom(),"page":page,"origin_y":origin}));
                    if let Some(folder)=&screenshots {
                        if let Err(e)=snapshot(&ui,&folder.join(format!("chrome-{step:02}.png"))) { run.errors.push(e.to_string()); }
                    }
                }
                use slint::platform::{WindowEvent,PointerEventButton};
                let point=slint::LogicalPosition::new((viewport.0+32.0)*0.5+171.0,viewport.1-43.0);
                let drag=|from:slint::LogicalPosition,to:slint::LogicalPosition,button| {
                    ui.window().dispatch_event(WindowEvent::PointerPressed{position:from,button});
                    ui.window().dispatch_event(WindowEvent::PointerMoved{position:to});
                    ui.window().dispatch_event(WindowEvent::PointerReleased{position:to,button});
                };
                match run.step {
                    0=>{run.chrome_viewport=Some(viewport);ui.set_top_hover(true);},
                    1=>{ui.set_top_hover(false);ui.set_bottom_hover(true);},
                    2=>{ui.set_bottom_hover(false);ui.set_show_toolbar(true);},
                    3=>ui.invoke_fullscreen(),
                    4=>ui.invoke_fullscreen(),
                    5=>{ui.set_show_toolbar(false);ui.set_top_hover(false);ui.set_bottom_hover(false);ui.set_show_settings(true);},
                    6=>{ui.set_show_settings(false);ui.set_show_toolbar(false);},
                    7=>{ui.set_bottom_hover(true);},
                    8=>ui.window().dispatch_event(WindowEvent::PointerScrolled{position:point,delta_x:0.0,delta_y:120.0}),
                    9=>drag(point,slint::LogicalPosition::new(point.x+56.0,point.y),PointerEventButton::Left),
                    10=>{
                        let left=(viewport.0+32.0-560.0)*0.5;
                        drag(slint::LogicalPosition::new(left+150.0,point.y),slint::LogicalPosition::new(left+319.0,point.y),PointerEventButton::Left);
                    },
                    11=>{
                        run.pan_origin=state.lock().unwrap().frame.as_ref().unwrap().origin.1;
                        let from=slint::LogicalPosition::new((viewport.0+32.0)*0.7,viewport.1*0.5);
                        drag(from,slint::LogicalPosition::new(from.x-80.0,from.y-60.0),PointerEventButton::Right);
                    },
                    12=>drag(point,point,PointerEventButton::Left),
                    13=>{ui.set_bottom_hover(false);},
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
