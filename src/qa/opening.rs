//! Native opening probes use the same picker result and file-drop routes as the window.
use super::*;
use crate::file_drop::FileDrop;
use anyhow::{Result, ensure};
use winit::event::WindowEvent;

const BOOK: &str = "tests/fixtures/core.pdf";
const OTHER: &str = "tests/fixtures/slides.pdf";

thread_local! {
    // The outer option is populated only for an intentional QA click. A
    // populated None reproduces cancelling the native picker.
    static PICKER_RESULT: RefCell<Option<Option<PathBuf>>> = const { RefCell::new(None) };
}

pub(crate) fn choose_file(native: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    PICKER_RESULT
        .with(|slot| slot.take())
        .unwrap_or_else(native)
}

pub(crate) fn opening_motion_policy() -> Option<bool> {
    // Test both Windows preferences in this process only, including on headless CI.
    if std::env::var_os("SAE_READER_QA_REPORT").is_some()
        && std::env::var("SAE_READER_QA_SCENARIO").as_deref() == Ok("opening")
    {
        return Some(std::env::var("SAE_READER_QA_OPEN_CASE").as_deref() != Ok("reduced"));
    }
    None
}

fn click_picker(ui: &ReaderWindow, result: Option<PathBuf>) -> Result<()> {
    use slint::platform::{PointerEventButton, WindowEvent};
    let (w, h) = (ui.get_viewport_width(), ui.get_viewport_height());
    let inset = (w.min(h) / 16.).clamp(24., 64.);
    let position = slint::LogicalPosition::new(inset + 35., h - inset - 19.);
    PICKER_RESULT.with(|slot| slot.replace(Some(result)));
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    ensure!(
        PICKER_RESULT.with(|slot| slot.take()).is_none(),
        "The visible choose-book button did not open the picker"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Start,
    Home,
    Failed,
    Opening,
    Settled,
    SeekingAnchor,
    ReadingDrop,
}

pub(super) struct Probe {
    case: String,
    stage: Stage,
    started: Instant,
    since: Instant,
    files: FileDrop,
    mix_frames: Vec<slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
    replaced: bool,
    settled_counts: (i32, i32),
}

impl Probe {
    pub(super) fn new() -> Self {
        Self {
            case: std::env::var("SAE_READER_QA_OPEN_CASE").unwrap_or_else(|_| "picker".into()),
            stage: Stage::Start,
            started: Instant::now(),
            since: Instant::now(),
            files: FileDrop::default(),
            mix_frames: Vec::new(),
            replaced: false,
            settled_counts: (0, 0),
        }
    }

    fn enter(&mut self, stage: Stage) {
        self.since = Instant::now();
        self.stage = stage;
    }

    pub(super) fn tick(
        &mut self,
        ui: &ReaderWindow,
        state: &Arc<Mutex<State>>,
        mailbox: &Arc<Mailbox>,
        screens: Option<&Path>,
        run: &mut Run,
    ) -> Result<bool> {
        ensure!(
            self.started.elapsed() < Duration::from_secs(20),
            "Opening probe timed out at {:?}",
            self.stage
        );
        let screens = screens.ok_or_else(|| anyhow::anyhow!("Opening screenshots required"))?;
        let frames = (ui.get_home_frames(), ui.get_home_reveal_frames());
        let active = ui.get_home_transition();
        let progress = ui.get_home_reveal_progress();
        let resources = ui.invoke_home_resources();
        let file_launch = matches!(self.case.as_str(), "direct" | "retry");
        if file_launch {
            ensure!(
                !ui.get_home_session() && !active && frames == (0, 0) && resources == 0,
                "File launch acquired the home or transition"
            );
        }
        let ready = |fixture: &str| {
            let s = state.lock().unwrap();
            ui.get_has_document()
                && s.frame.is_some()
                && s.generation == s.completed_generation
                && s.request
                    .as_ref()
                    .is_some_and(|r| r.path.ends_with(fixture))
        };
        match self.stage {
            Stage::Start => {
                fs::create_dir_all(screens)?;
                ui.window().set_size(slint::LogicalSize::new(640., 480.));
                if self.case == "retry" {
                    self.enter(Stage::Failed);
                } else if self.case == "direct" {
                    self.enter(Stage::Opening);
                } else {
                    ensure!(
                        matches!(
                            self.case.as_str(),
                            "picker" | "drop" | "static" | "reduced" | "replace"
                        ),
                        "Unknown opening case"
                    );
                    ui.window().with_winit_window(|w| w.focus_window());
                    self.enter(Stage::Home);
                }
            }
            Stage::Home => {
                // Windows can enforce a larger physical minimum on a scaled
                // monitor. All capture positions come from the actual viewport.
                if self.since.elapsed() < Duration::from_millis(150) {
                    return Ok(false);
                }
                ensure!(
                    ui.get_home_session() && !ui.get_has_document() && !active,
                    "Missing idle cover"
                );
                if self.case == "picker" {
                    let generation = state.lock().unwrap().generation;
                    click_picker(ui, None)?;
                    ensure!(
                        ui.get_home_session() && generation == state.lock().unwrap().generation,
                        "Cancelling the picker submitted a book"
                    );
                    click_picker(ui, Some(screens.join("missing-fixture.pdf")))?;
                    self.enter(Stage::Failed);
                } else {
                    if self.case == "drop" {
                        let generation = state.lock().unwrap().generation;
                        self.files.handle(
                            &WindowEvent::HoveredFile(BOOK.into()),
                            ui,
                            state,
                            mailbox,
                        );
                        ensure!(
                            ui.get_book_drag_over(),
                            "Supported drag did not light the choose-book button"
                        );
                        self.files
                            .handle(&WindowEvent::HoveredFileCancelled, ui, state, mailbox);
                        ensure!(
                            generation == state.lock().unwrap().generation
                                && !ui.get_book_drag_over(),
                            "Leaving a drag kept the highlight or opened a book"
                        );
                        self.drop_batch(ui, state, mailbox, &["Cargo.toml"]);
                        ensure!(
                            generation == state.lock().unwrap().generation
                                && !ui.get_book_drag_over(),
                            "Unsupported batch opened the reader"
                        );
                        self.drop_batch(ui, state, mailbox, &["Cargo.toml", BOOK, OTHER]);
                        ensure!(
                            state.lock().unwrap().generation == generation + 1
                                && !ui.get_book_drag_over(),
                            "Batch left the highlight or opened multiple books"
                        );
                    } else if self.case == "replace" {
                        click_picker(ui, Some(BOOK.into()))?;
                        ensure!(
                            ui.get_opening_document() && ui.get_home_session(),
                            "No waiting cover"
                        );
                        click_picker(ui, Some(OTHER.into()))?;
                    } else {
                        click_picker(ui, Some(BOOK.into()))?;
                    }
                    snapshot(ui, &screens.join("frozen.png"))?;
                    self.enter(Stage::Opening);
                }
            }
            Stage::Failed => {
                if ui.get_opening_document() || !ui.get_status().starts_with("无法读取") {
                    return Ok(false);
                }
                ensure!(
                    ui.get_home_session() != file_launch
                        && !ui.get_has_document()
                        && !active
                        && frames.1 == 0,
                    "Failed opening changed the cover session or played a transition"
                );
                run.snapshots.push(json!({"failed_open_recovered":true}));
                if file_launch {
                    ensure!(
                        ui.get_home_picture().size().width == 0,
                        "A bad file loaded cover art"
                    );
                    snapshot(ui, &screens.join("recovery.png"))?;
                    PICKER_RESULT.with(|slot| slot.replace(Some(Some(BOOK.into()))));
                    ui.invoke_open_file();
                } else {
                    click_picker(ui, Some(BOOK.into()))?;
                    snapshot(ui, &screens.join("frozen.png"))?;
                }
                self.enter(Stage::Opening);
            }
            Stage::Opening => {
                if active {
                    ensure!(
                        ui.get_has_document() && ui.get_pages().row_count() != 0,
                        "The opening effect ran before a real page was assigned"
                    );
                    if self.case == "drop" {
                        let s = state.lock().unwrap();
                        ensure!(
                            s.frame.as_ref().is_some_and(|frame| frame.page == 2)
                                && s.metrics.frames.iter().all(|frame| frame["page"] == 2),
                            "Opening exposed a different page before the saved position"
                        );
                    }
                    if self.case == "replace" && !self.replaced {
                        ensure!(ready(OTHER), "A superseded result started the effect");
                        self.replaced = true;
                        self.drop_batch(ui, state, mailbox, &[BOOK]);
                    } else if matches!(self.case.as_str(), "picker" | "drop") {
                        // Winit may wake a fraction before a 30 ms boundary.
                        // Only the first completed exposure can contain grains.
                        let threshold = [1. / 9., 0.29, 0.68];
                        if self.mix_frames.len() < threshold.len()
                            && progress >= threshold[self.mix_frames.len()]
                        {
                            ensure!(
                                ui.get_home_reveal_frame().size().width > 0,
                                "Empty active transition texture"
                            );
                            // Keep PNG encoding off the 270 ms animation timeline.
                            self.mix_frames.push(ui.window().take_snapshot()?);
                            run.snapshots.push(json!({"mix":self.mix_frames.len(),"progress":progress,"frames":frames.1}));
                        }
                    } else {
                        ensure!(
                            self.case == "replace",
                            "Static or direct opening played an effect"
                        );
                    }
                    return Ok(false);
                }
                if !ready(BOOK)
                    || ui.get_home_session()
                    || self.since.elapsed() < Duration::from_millis(150)
                {
                    return Ok(false);
                }
                match self.case.as_str() {
                    "picker" | "drop" => {
                        ensure!(
                            self.mix_frames.len() >= 2,
                            "Missed visible stages of the opening"
                        )
                    }
                    "replace" => ensure!(
                        self.replaced,
                        "Supersession was not exercised during the effect"
                    ),
                    _ => ensure!(
                        frames == (0, 0),
                        "Reduced motion, static or direct opening built animation frames"
                    ),
                }
                ensure!(
                    resources == 0
                        && ui.get_home_picture().size().width == 0
                        && ui.get_home_frame().size().width == 0
                        && ui.get_home_reveal_frame().size().width == 0
                        && progress.abs() < 0.001,
                    "Opening retained timers, images or its mask"
                );
                for (index, pixels) in self.mix_frames.drain(..).enumerate() {
                    write_snapshot(&pixels, &screens.join(format!("mix-{}.png", index + 1)))?;
                }
                if self.case == "drop" {
                    ensure!(
                        state
                            .lock()
                            .unwrap()
                            .frame
                            .as_ref()
                            .is_some_and(|frame| frame.page == 2),
                        "Restored page changed after the reveal"
                    );
                    run.snapshots.push(json!({"resumed_page":3}));
                }
                snapshot(ui, &screens.join("book.png"))?;
                run.painted_ms = Some(self.started.elapsed().as_secs_f64() * 1000.);
                run.first_counters = Some(counters());
                self.settled_counts = frames;
                self.enter(Stage::Settled);
            }
            Stage::Settled => {
                if self.since.elapsed() < Duration::from_millis(420) {
                    return Ok(false);
                }
                ensure!(
                    frames == self.settled_counts && resources == 0 && !active,
                    "Opening kept ticking"
                );
                run.snapshots.push(json!({"idle_home_frames":frames.0-self.settled_counts.0,
                    "idle_reveal_frames":frames.1-self.settled_counts.1,"reveal_frames":frames.1,"resources":resources}));
                if self.case != "direct" {
                    return Ok(true);
                }
                ensure!(
                    ui.invoke_jump_page("3".into()),
                    "Could not seed the saved reading position"
                );
                self.enter(Stage::SeekingAnchor);
            }
            Stage::SeekingAnchor => {
                if !ready(BOOK)
                    || !state
                        .lock()
                        .unwrap()
                        .frame
                        .as_ref()
                        .is_some_and(|frame| frame.page == 2)
                {
                    return Ok(false);
                }
                snapshot(ui, &screens.join("resumed-page.png"))?;
                // Opening a different book persists the source anchor via the regular save path.
                self.drop_batch(ui, state, mailbox, &[OTHER]);
                self.enter(Stage::ReadingDrop);
            }
            Stage::ReadingDrop => {
                if !ready(OTHER) {
                    return Ok(false);
                }
                snapshot(ui, &screens.join("reading-drop.png"))?;
                run.snapshots.push(json!({"reading_drop_direct":true}));
                return Ok(true);
            }
        }
        run.operations += 1;
        Ok(false)
    }

    fn drop_batch(
        &mut self,
        ui: &ReaderWindow,
        state: &Arc<Mutex<State>>,
        mailbox: &Arc<Mailbox>,
        paths: &[&str],
    ) {
        // Winit's Windows OLE target delivers all hover events before all drops, in shell order.
        for path in paths {
            self.files.handle(
                &WindowEvent::HoveredFile(PathBuf::from(path)),
                ui,
                state,
                mailbox,
            );
        }
        for path in paths {
            self.files.handle(
                &WindowEvent::DroppedFile(PathBuf::from(path)),
                ui,
                state,
                mailbox,
            );
        }
    }
}
