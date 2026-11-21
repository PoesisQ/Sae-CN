use super::{ReaderWindow, Run, State};
use serde_json::json;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::sync::{Arc, Mutex};

/// Send captured left-button gestures to the native page, including a final release.
fn drag(ui: &ReaderWindow, from: LogicalPosition, to: LogicalPosition) {
    let window = ui.window();
    let button = PointerEventButton::Left;
    window.dispatch_event(WindowEvent::PointerMoved { position: from });
    window.dispatch_event(WindowEvent::PointerPressed {
        position: from,
        button,
    });
    window.dispatch_event(WindowEvent::PointerMoved { position: to });
    window.dispatch_event(WindowEvent::PointerReleased {
        position: to,
        button,
    });
}

pub(super) fn step(ui: &ReaderWindow, state: &Arc<Mutex<State>>, run: &mut Run) -> bool {
    let n = run.step;
    let corners = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
    let columns = ui.get_grid_columns().max(1);
    let visible = ui.get_visible_count().max(1);
    let w = ui.get_viewport_width() / columns as f32;
    let h = ui.get_viewport_height() / ui.get_grid_rows().max(1) as f32;
    let mid = LogicalPosition::new(w * 0.5, h * 0.5);
    {
        let s = state.lock().unwrap();
        let f = s.frame.as_ref().unwrap();
        let pan = s.request.as_ref().unwrap().view.pan;
        run.snapshots
            .push(json!({"step":n, "pages":visible, "page":f.page,
            "pan":pan,"origin":f.origin,"overflow":f.overflow,"generation":s.generation,
            "selected":!s.selected.is_empty(),"can_return":ui.get_can_go_back()}));
        if (2..=5).contains(&n) {
            let expected = corners[n - 2];
            let tiles = std::iter::once(f)
                .chain(s.right.iter())
                .chain(s.extra.iter());
            for tile in tiles {
                let observed = [tile.origin.0 / tile.dpi, tile.origin.1 / tile.dpi];
                for (axis, overflow) in [tile.overflow.0, tile.overflow.1].into_iter().enumerate() {
                    if overflow < 1. || (observed[axis] - expected[axis] * overflow).abs() > 1.1 {
                        run.errors.push(format!("visible page {} missed corner {expected:?}: origin {observed:?} extent {:?}", tile.page, tile.overflow));
                    }
                }
            }
            if f.page != 0 || !s.selected.is_empty() {
                run.errors
                    .push("plain drag selected text or turned the page".into());
            }
        }
        if n == 6 && Some(s.generation) != run.panel_generation {
            run.errors
                .push("dragging past a clamped edge queued another render".into());
        }
        if n == 7 && (s.selected.is_empty() || pan != corners[3]) {
            run.errors
                .push("Shift-drag lost the text selection or panned the page".into());
        }
        if n == 8 && (f.page == 0 || ui.get_can_go_back()) {
            run.errors
                .push("a short page click failed to turn normally".into());
        }
        if n == 9 && (f.page != 0 || !ui.get_can_go_back()) {
            run.errors
                .push("outline jump had no return position".into());
        }
        if n == 10 && (Some(f.page) != run.navigation_expected || ui.get_can_go_back()) {
            run.errors
                .push("return from outline did not restore position".into());
        }
    }
    match n {
        0 => {
            ui.invoke_typography("zoom".into(), 4.0);
        }
        1..=5 => {
            if n == 5 {
                run.panel_generation = Some(state.lock().unwrap().generation);
            }
            let [x, y] = corners[(n - 1).min(3)];
            let i = if n % 2 == 0 { visible - 1 } else { 0 };
            let from = LogicalPosition::new(
                mid.x + (i % columns) as f32 * w,
                mid.y + (i / columns) as f32 * h,
            );
            let to = LogicalPosition::new(
                from.x + (1. - 2. * x) * w * 0.34,
                from.y + (1. - 2. * y) * h * 0.34,
            );
            for _ in 0..15 {
                drag(ui, from, to);
            }
        }
        6 => {
            ui.window().dispatch_event(WindowEvent::KeyPressed {
                text: Key::Shift.into(),
            });
            drag(ui, mid, LogicalPosition::new(mid.x + 130., mid.y + 24.));
            ui.window().dispatch_event(WindowEvent::KeyReleased {
                text: Key::Shift.into(),
            });
        }
        7 => {
            let p = LogicalPosition::new(ui.get_viewport_width() - w * 0.18, mid.y);
            drag(ui, p, p);
        }
        8 => {
            run.navigation_expected = state.lock().unwrap().frame.as_ref().map(|f| f.page);
            if let Some(row) = ui.get_contents().row_data(0) {
                ui.invoke_follow_link(row.uri);
            } else {
                run.errors
                    .push("fixture needs an outline destination".into());
            }
        }
        9 => ui.invoke_back(),
        _ => return true,
    }
    false
}
