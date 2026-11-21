//! Exercise the visible PDF detents through Slint's native pointer path.
use super::{Mailbox, ReaderWindow, Run, State, Stop, open, snapshot, write_snapshot};
use anyhow::{Result, ensure};
use serde_json::json;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize};
use std::path::Path;
use std::sync::{Arc, Mutex};

struct Case {
    file: &'static str,
    size: (f32, f32),
    marks: &'static [(Stop, f32)],
    gap: f32,
}

const CASES: [Case; 3] = [
    Case {
        file: "slides.pdf",
        size: (960., 540.),
        marks: &[(Stop::Page, 0.)],
        gap: -36.,
    },
    Case {
        file: "core.pdf",
        size: (1180., 840.),
        marks: &[(Stop::Page, -36.), (Stop::Width, 36.)],
        gap: 0.,
    },
    Case {
        file: "mixed-pages.pdf",
        size: (1180., 840.),
        marks: &[(Stop::Page, -49.), (Stop::Width, 0.), (Stop::Fill, 49.)],
        gap: 90.,
    },
];

#[derive(Default)]
enum Stage {
    #[default]
    Open,
    Raise,
    Gap,
    Press,
    Check,
}

#[derive(Default)]
pub(super) struct Probe {
    case: usize,
    slot: usize,
    stage: Stage,
}

fn point(ui: &ReaderWindow, angle: f32) -> LogicalPosition {
    let radius = ui.get_knob_radius() * 0.885;
    let (s, c) = angle.to_radians().sin_cos();
    LogicalPosition::new(
        ui.get_knob_center_x() + radius * s,
        ui.get_knob_center_y() + radius * c,
    )
}

fn click(ui: &ReaderWindow, position: LogicalPosition) {
    let w = ui.window();
    let button = PointerEventButton::Left;
    w.dispatch_event(WindowEvent::PointerMoved { position });
    w.dispatch_event(WindowEvent::PointerPressed { position, button });
    w.dispatch_event(WindowEvent::PointerReleased { position, button });
    w.dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(ui.get_knob_center_x(), ui.get_knob_center_y()),
    });
}

fn readout(
    ui: &ReaderWindow,
    pixels: &slint::SharedPixelBuffer<slint::Rgba8Pixel>,
) -> blake3::Hash {
    let dpi = pixels.width() as f32 / ui.get_viewport_width();
    let (x, y) = (ui.get_readout_x(), ui.get_readout_y());
    let left = ((x - 20.) * dpi) as usize;
    let right = ((x + 20.) * dpi) as usize;
    let mut hash = blake3::Hasher::new();
    for row in ((y - 7.) * dpi) as usize..((y + 7.) * dpi) as usize {
        let offset = row * pixels.width() as usize;
        hash.update(&pixels.as_bytes()[(offset + left) * 4..(offset + right) * 4]);
    }
    hash.finalize()
}

impl Probe {
    pub(super) fn tick(
        &mut self,
        ui: &ReaderWindow,
        state: &Arc<Mutex<State>>,
        mailbox: &Arc<Mailbox>,
        screens: &Path,
        run: &mut Run,
    ) -> Result<bool> {
        let c = &CASES[self.case];
        let count = c.marks.len();
        match self.stage {
            Stage::Open => {
                ui.window().set_size(LogicalSize::new(c.size.0, c.size.1));
                open(Path::new("tests/fixtures").join(c.file), ui, state, mailbox);
                let multi = count == 3;
                ui.set_double_page(multi);
                ui.invoke_mode_changed(multi);
                if multi {
                    ui.set_pdf_multi(true);
                    ui.invoke_multi_mode();
                    ui.set_grid_choice(4);
                    ui.invoke_grid_changed(4);
                }
                self.stage = Stage::Raise;
            }
            Stage::Raise => {
                ui.invoke_jump_page("1".into());
                ui.invoke_typography("zoom".into(), 1.27);
                ui.set_zoom_pinned(true);
                self.stage = Stage::Gap;
            }
            Stage::Gap => {
                let at = ui.get_pdf_presets().count;
                ensure!(
                    at == count as i32 && ui.get_knob_radius() > 33.9,
                    "expected {count} distinct marks on the raised dial, got {at} at {}×{}",
                    ui.get_viewport_width(),
                    ui.get_viewport_height()
                );
                snapshot(ui, &screens.join(format!("{count}-marks.png")))?;
                click(ui, point(ui, c.gap));
                self.stage = Stage::Press;
            }
            Stage::Press => {
                if self.slot == 0 {
                    ensure!(
                        ui.get_pdf_zoom_stop() == Stop::Free as i32
                            && (ui.get_pdf_zoom() - 1.27).abs() < 0.001,
                        "an unmarked point on the {count}-stop dial selected a zoom"
                    );
                }
                let (stop, angle) = c.marks[self.slot];
                let p = point(ui, angle);
                let number = readout(ui, &ui.window().take_snapshot()?);
                ui.window()
                    .dispatch_event(WindowEvent::PointerMoved { position: p });
                let hover = ui.window().take_snapshot()?;
                ensure!(
                    readout(ui, &hover) != number,
                    "hovering {stop:?} left the numeric readout on screen"
                );
                write_snapshot(&hover, &screens.join(format!("{count}-hover-{stop:?}.png")))?;
                click(ui, p);
                self.stage = Stage::Check;
            }
            Stage::Check => {
                let (stop, angle) = c.marks[self.slot];
                let (columns, rows) = (ui.get_grid_columns(), ui.get_grid_rows());
                let (w, h) = (ui.get_viewport_width(), ui.get_viewport_height());
                let (s, p) = (state.lock().unwrap(), point(ui, angle));
                let request = s.request.as_ref().unwrap();
                ensure!(
                    ui.get_pdf_zoom_stop() == stop as i32
                        && Stop::from_fit(request.view.fit_width, request.view.fit_cover) == stop,
                    "click on {count}-mark dial at {angle}° did not select {stop:?}"
                );
                let mut scales = Vec::new();
                let mut sum = 0.;
                for f in s.frame.iter().chain(&s.right).chain(&s.extra) {
                    let wide = (w / columns as f32 / f.page_size.0) * f.dpi;
                    let tall = (h / rows as f32 / f.page_size.1) * f.dpi;
                    let wanted = match stop {
                        Stop::Width => wide,
                        Stop::Fill => wide.max(tall),
                        _ => wide.min(tall),
                    }
                    .clamp(0.02, 16.);
                    ensure!(
                        (f.scale - wanted).abs() * f.page_size.0.max(f.page_size.1) < 0.55,
                        "{stop:?} drew tile {} at {}, expected {wanted}",
                        f.page,
                        f.scale
                    );
                    sum += wanted / wide.min(tall).clamp(0.02, 16.);
                    scales.push(f.scale);
                }
                ensure!(
                    scales.len() == if count == 3 { 6 } else { 1 },
                    "test spread did not display its expected tiles"
                );
                ensure!(
                    (ui.get_pdf_zoom() - sum / scales.len() as f32).abs() < 0.001,
                    "the readout disagrees with rendered {stop:?} tiles"
                );
                drop(s);
                // Check the actual selected ink at the independently specified
                // bearing. Moving to the hub first removes hover highlighting.
                let pixels = ui.window().take_snapshot()?;
                let density = pixels.width() as f32 / w;
                let (cx, cy) = (
                    (p.x * density).round() as i32,
                    (p.y * pixels.height() as f32 / h).round() as i32,
                );
                let reach = (2. * density).ceil() as i32;
                let ink = (cy - reach..=cy + reach).any(|y| {
                    (cx - reach..=cx + reach).any(|x| {
                        let i = (y as usize * pixels.width() as usize + x as usize) * 4;
                        let [r, g, b, _] = pixels.as_bytes()[i..i + 4] else {
                            return false;
                        };
                        i16::from(r) - i16::from(g) > 40 && i16::from(g) - i16::from(b) > 8
                    })
                });
                ensure!(ink, "selected {stop:?} tick is missing at {angle}°");
                write_snapshot(
                    &pixels,
                    &screens.join(format!("{count}-selected-{stop:?}.png")),
                )?;
                run.snapshots.push(json!({"marks":count,"stop":format!("{stop:?}"),"bearing":angle,"center":[p.x,p.y],"zoom":ui.get_pdf_zoom(),"scales":scales,"grid":[columns,rows]}));
                self.slot += 1;
                if self.slot == count {
                    self.case += 1;
                    if self.case == CASES.len() {
                        return Ok(true);
                    }
                    self.slot = 0;
                    self.stage = Stage::Open;
                } else {
                    self.stage = Stage::Press;
                }
            }
        }
        Ok(false)
    }
}
