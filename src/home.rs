//! Standalone launches own one foreground-only cover session. Document launches
//! never acquire a cover, open its state marker, or allocate a pixel buffer.
mod field;
mod wordmark;
use crate::ReaderWindow;
use field::{Field, Kind};
use slint::winit_030::{WinitWindowAccessor, winit::event::WindowEvent};
use slint::{ComponentHandle, Image, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    fs::{self, OpenOptions},
    io::ErrorKind,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(action: u32, param: u32, output: *mut i32, flags: u32) -> i32;
}

fn allows_motion() -> bool {
    let mut enabled = 0i32;
    // SPI_GETCLIENTAREAANIMATION writes a Win32 BOOL.
    unsafe { SystemParametersInfoW(0x1042, 0, &mut enabled, 0) != 0 && enabled != 0 }
}

// Read small patches on the actual strokes. A trimmed mean rejects individual
// grains and a bright counter in the "a" without scanning or copying a frame.
fn cover_tone(frame: Image, x: f32, y: f32, w: f32, h: f32) -> i32 {
    let size = frame.size();
    if size.width == 0 || size.height == 0 {
        return 128;
    }
    let sample = |bytes: &[u8], stride: usize| {
        let positions = [
            (0.12, 0.23),
            (0.29, 0.19),
            (0.15, 0.48),
            (0.27, 0.62),
            (0.12, 0.90),
            (0.46, 0.44),
            (0.60, 0.53),
            (0.59, 0.76),
            (0.47, 0.88),
            (0.64, 0.91),
            (0.74, 0.65),
            (0.80, 0.55),
            (0.87, 0.53),
            (0.78, 0.82),
            (0.87, 0.90),
        ];
        let mut tones = positions.map(|(u, v)| {
            let mut sum = 0;
            for (dx, dy) in [(-0.012, -0.012), (0.0, 0.013), (0.012, -0.006)] {
                let px = (((x + w * (u + dx)) * size.width as f32) as usize)
                    .min(size.width as usize - 1);
                let py = (((y + h * (v + dy)) * size.height as f32) as usize)
                    .min(size.height as usize - 1);
                let offset = (py * size.width as usize + px) * stride;
                sum += 54 * i32::from(bytes[offset])
                    + 183 * i32::from(bytes[offset + 1])
                    + 19 * i32::from(bytes[offset + 2]);
            }
            sum / (3 * 256)
        });
        tones.sort_unstable();
        tones[3..12].iter().sum::<i32>() / 9
    };
    if let Some(pixels) = frame.to_rgb8() {
        return sample(pixels.as_bytes(), 3);
    }
    // Only the small built-in stills use Slint's embedded RGBA representation.
    frame
        .to_rgba8()
        .map_or(128, |pixels| sample(pixels.as_bytes(), 4))
}

fn alternate(config: &Path) -> i32 {
    let marker = config.with_file_name("next-cover-blue");
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    // Consuming the marker commits the Blue slot. A racing launch that lost the
    // removal retries so that it can claim the following Plasma slot instead.
    for _ in 0..4 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
        {
            Ok(_) => return 1,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => match fs::remove_file(&marker) {
                Ok(()) => return 2,
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(_) => break,
            },
            Err(_) => break,
        }
    }
    1
}

pub(crate) struct Cover {
    timer: Timer,
    field: RefCell<Option<Field>>,
    stamp: RefCell<Option<wordmark::Stamp>>,
    kind: Cell<i32>,
    period: Option<Duration>,
    started: Cell<Option<Instant>>,
    played: Cell<Duration>,
    last_input: Cell<Instant>,
    focused: Cell<bool>,
    occluded: Cell<bool>,
    picker: Cell<bool>,
    retired: Cell<bool>,
}

impl Cover {
    pub fn attach(ui: &ReaderWindow, config: &Path) -> Rc<Self> {
        ui.on_cover_tone(cover_tone);
        let mut forced = None;
        let mut period = Some(Duration::from_millis(67));
        for arg in std::env::args_os().skip(1) {
            match arg.to_str() {
                Some("--cover=plasma") => forced = Some(1),
                Some("--cover=blue") => forced = Some(2),
                Some("--cover=print") => forced = Some(0),
                Some("--motion=static") => period = None,
                Some("--motion=30") => period = Some(Duration::from_micros(33_333)),
                _ => {}
            }
        }
        let kind = forced.unwrap_or_else(|| alternate(config));
        let cover = Rc::new(Self {
            timer: Timer::default(),
            field: RefCell::default(),
            stamp: RefCell::default(),
            kind: Cell::new(kind),
            period,
            started: Cell::new(None),
            played: Cell::new(Duration::ZERO),
            last_input: Cell::new(Instant::now()),
            focused: Cell::new(false),
            occluded: Cell::new(false),
            picker: Cell::new(false),
            retired: Cell::new(false),
        });
        let owner = Rc::downgrade(&cover);
        ui.on_negative_mark(move |frame, rect, monochrome| {
            owner.upgrade().map_or_else(Image::default, |home| {
                wordmark::render(&mut home.stamp.borrow_mut(), frame, rect, monochrome)
            })
        });
        ui.set_home_session(true);
        ui.set_home_variant(kind);
        let weak = Rc::downgrade(&cover);
        let window = ui.as_weak();
        ui.on_cycle_home(move |direction| {
            if let (Some(home), Some(ui)) = (weak.upgrade(), window.upgrade()) {
                home.pause();
                home.field.replace(None);
                let kind = (home.kind.get() + direction).rem_euclid(3);
                if kind == 0 {
                    home.stamp.replace(None);
                }
                home.kind.set(kind);
                home.played.set(Duration::ZERO);
                home.last_input.set(Instant::now());
                ui.set_home_frame(Image::default());
                ui.set_home_variant(kind);
                home.resume(&ui);
            }
        });
        let weak = Rc::downgrade(&cover);
        let window = ui.as_weak();
        ui.on_home_picker(move |open| {
            if let (Some(home), Some(ui)) = (weak.upgrade(), window.upgrade()) {
                home.picker.set(open);
                if open {
                    home.pause();
                } else {
                    home.last_input.set(Instant::now());
                    home.resume(&ui);
                }
            }
        });
        let weak = Rc::downgrade(&cover);
        let window = ui.as_weak();
        ui.on_retire_home(move || {
            if let (Some(home), Some(ui)) = (weak.upgrade(), window.upgrade()) {
                home.retired.set(true);
                home.stamp.replace(None);
                home.pause();
                ui.set_home_session(false);
                ui.set_home_frame(Image::default());
                home.field.replace(None);
                ui.set_home_variant(0);
            }
        });
        cover
    }
    fn pause(&self) {
        self.timer.stop();
        if let Some(t) = self.started.take() {
            self.played.set(self.played.get() + t.elapsed());
        }
    }
    pub fn shown(self: &Rc<Self>, ui: &ReaderWindow) {
        self.focused.set(
            ui.window()
                .with_winit_window(|w| w.has_focus())
                .unwrap_or(false),
        );
        self.resume(ui);
    }
    fn resume(self: &Rc<Self>, ui: &ReaderWindow) {
        let Some(period) = self.period else { return };
        if self.retired.get()
            || self.kind.get() == 0
            || self.timer.running()
            || !self.focused.get()
            || self.occluded.get()
            || self.picker.get()
            || !ui.window().is_visible()
            || ui
                .window()
                .with_winit_window(|w| w.is_minimized() == Some(true))
                .unwrap_or(true)
            || !allows_motion()
        {
            return;
        }
        self.started.set(Some(Instant::now()));
        let weak = Rc::downgrade(self);
        let window = ui.as_weak();
        self.timer.start(TimerMode::Repeated, period, move || {
            if let (Some(home), Some(ui)) = (weak.upgrade(), window.upgrade()) {
                // Slint may already have queued this tick when a picker or document
                // retires the cover in the same event-loop turn.
                let Some(started) = home.started.get() else { return };
                if home.last_input.get().elapsed() >= Duration::from_secs(60)
                    || !ui.window().is_visible()
                    || ui
                        .window()
                        .with_winit_window(|w| w.is_minimized() == Some(true))
                        .unwrap_or(true)
                {
                    home.pause();
                    return;
                }
                let size = ui.window().size();
                let size = Field::dimensions(size.width, size.height);
                let mut field = home.field.borrow_mut();
                if field.as_ref().is_none_or(|f| f.size != size) {
                    *field = Field::new(
                        if home.kind.get() == 1 {
                            Kind::Plasma
                        } else {
                            Kind::Blue
                        },
                        size,
                    );
                }
                let elapsed = home.played.get() + started.elapsed();
                if let Some(frame) = field.as_mut().and_then(|f| f.render(elapsed.as_secs_f32())) {
                    ui.set_home_frame(frame);
                    #[cfg(feature = "qa")]
                    ui.set_home_frames(ui.get_home_frames() + 1);
                } else {
                    home.pause();
                    *field = None;
                    ui.set_home_frame(Image::default());
                }
            }
        });
    }

    pub fn event(self: &Rc<Self>, ui: &ReaderWindow, event: &WindowEvent) {
        if self.retired.get() {
            return;
        }
        match event {
            WindowEvent::Focused(focus) => {
                self.focused.set(*focus);
                if !focus {
                    self.pause();
                    return;
                }
            }
            WindowEvent::Occluded(hidden) => {
                self.occluded.set(*hidden);
                if *hidden {
                    self.pause();
                    return;
                }
            }
            WindowEvent::Resized(size) if size.width == 0 || size.height == 0 => {
                self.pause();
                return;
            }
            WindowEvent::Resized(_)
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::CursorMoved { .. } => {}
            _ => return,
        }
        self.last_input.set(Instant::now());
        if !self.timer.running() {
            self.resume(ui);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_launches_alternate_without_touching_book_preferences() {
        let dir = std::env::temp_dir().join(format!("sae-cover-{}", std::process::id()));
        let config = dir.join("state.json");
        let _ = std::fs::remove_file(dir.join("next-cover-blue"));
        assert_eq!(
            [
                alternate(&config),
                alternate(&config),
                alternate(&config),
                alternate(&config)
            ],
            [1, 2, 1, 2]
        );
        assert!(!config.exists());
        let _ = fs::remove_dir(dir);
    }
    #[test]
    fn both_fields_change_and_blue_retraces_the_same_frames() {
        for kind in [Kind::Plasma, Kind::Blue] {
            let mut f = Field::new(kind, (160, 90)).unwrap();
            let pixels = |f: &mut Field, t| f.render(t).unwrap().to_rgb8().unwrap();
            let a = pixels(&mut f, 0.0);
            let b = pixels(&mut f, 12.0);
            assert_ne!(a.as_bytes(), b.as_bytes());
            if kind == Kind::Blue {
                let mirrored = 2. * field::BLUE_ONE_WAY_SECONDS - 12.;
                assert_eq!(b.as_bytes(), pixels(&mut f, mirrored).as_bytes());
                assert_eq!(
                    a.as_bytes(),
                    pixels(&mut f, 2. * field::BLUE_ONE_WAY_SECONDS).as_bytes()
                );
            }
        }
    }

    #[test]
    #[ignore = "requires an explicit local reference output directory"]
    fn render_reference_frames() {
        use std::{io::Write, time::Instant};
        let dir = std::path::PathBuf::from(std::env::var_os("SAE_HOME_REFERENCE_DIR").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        for (name, kind) in [("plasma", Kind::Plasma), ("blue", Kind::Blue)] {
            let mut f = Field::new(kind, (640, 360)).unwrap();
            for (i, t) in [0., 1.23, 4.35, 6.67, 9.12, 12., 14.33, 18.47, 22.75, 26.]
                .into_iter()
                .enumerate()
            {
                let p = f.render(t).unwrap().to_rgb8().unwrap();
                let mut out = std::io::BufWriter::new(
                    std::fs::File::create(dir.join(format!("{name}-{i}.ppm"))).unwrap(),
                );
                out.write_all(b"P6\n640 360\n255\n").unwrap();
                out.write_all(p.as_bytes()).unwrap();
            }
            let mut times = Vec::new();
            for i in 0..150 {
                let start = Instant::now();
                std::hint::black_box(f.render(i as f32 / 15.).unwrap());
                times.push(start.elapsed().as_secs_f64() * 1000.);
            }
            std::fs::write(
                dir.join(format!("{name}-times.json")),
                serde_json::to_vec(&times).unwrap(),
            )
            .unwrap();
        }
    }
}
